use std::{
    result,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use error::{Error, METHOD_NOT_FOUND};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use tiny_http::Response as HttpResponse;
use tiny_http::Server;

pub mod error;

pub type Result<T> = result::Result<T, Error>;

// re-export
pub use tiny_http;

pub struct JsonRpcServer {
    server: Arc<Server>,
    handles: Vec<JoinHandle<Result<()>>>,
}

impl JsonRpcServer {
    ///
    pub fn new<F, T>(server: Server, state: Arc<Mutex<T>>, func: F) -> Self
    where
        F: Fn(Request, Arc<Mutex<T>>) -> Result<Response> + Clone + Send + Sync + 'static,
        T: Send + 'static,
    {
        Self::run(Arc::new(server), state, func)
    }

    ///
    pub fn server_addr(&self) -> tiny_http::ListenAddr {
        self.server.server_addr()
    }

    ///
    pub fn port(&self) -> Option<u16> {
        self.server.server_addr().to_ip().map(|addr| addr.port())
    }

    ///
    fn run<F, T>(server: Arc<Server>, state: Arc<Mutex<T>>, func: F) -> Self
    where
        F: Fn(Request, Arc<Mutex<T>>) -> Result<Response> + Clone + Send + Sync + 'static,
        T: Send + 'static,
    {
        // todo config the number of threads
        let mut handles = Vec::with_capacity(4);

        for _ in 0..4 {
            let server = server.clone();
            let func = func.clone();
            let state = state.clone();
            let handle = thread::spawn(move || {
                loop {
                    // receive http request
                    let mut http_request = match server.recv() {
                        Ok(request) => request,
                        Err(err) => {
                            // not much to do if recv fails
                            tracing::error!("recv error: {}", err);
                            continue;
                        }
                    };

                    // validate/parse the request
                    let response = match validate_request(&mut http_request) {
                        Ok(request) => {
                            // handle the request
                            let id = request.id.clone();
                            match handle_request(request, state.clone(), func.clone()) {
                                Ok(response) => response,
                                Err(err) => Response::from_error(id, err),
                            }
                        }
                        Err(err) => {
                            // no id since we couldn't validate the request...
                            Response::from_error(None, err)
                        }
                    };

                    // send the response
                    if let Err(err) = send_response(http_request, response) {
                        tracing::error!("send_response error: {}", err);
                    }
                }
            });
            handles.push(handle);
        }
        Self { server, handles }
    }

    pub fn join_threads(&mut self) {
        while let Some(handle) = self.handles.pop() {
            let _ = handle.join();
        }
    }
}

fn validate_request(http_request: &mut tiny_http::Request) -> Result<Request> {
    tracing::debug!(
        "received request - method: {:?}, url: {:?}, headers: {:?}",
        http_request.method(),
        http_request.url(),
        http_request.headers()
    );

    // check content-type header exists
    let content_header = http_request
        .headers()
        .iter()
        .find(|h| h.field.as_str().as_str() == "Content-Type")
        .ok_or(error::Error::NoContentType)?;

    // check content-type is application/json
    if content_header.value.as_str() != "application/json" {
        return Err(error::Error::WrongContentType);
    }

    // parse json into request
    let mut s = String::new(); // todo: performance
    http_request.as_reader().read_to_string(&mut s)?;

    let request: Request = serde_json::from_str(&s)?;

    Ok(request)
}

fn handle_request<F, T>(request: Request, state: Arc<Mutex<T>>, process: F) -> Result<Response>
where
    F: Fn(Request, Arc<Mutex<T>>) -> Result<Response> + Clone + Send + Sync + 'static,
    T: Send + 'static,
{
    // check jsonrpc version
    if request.jsonrpc.as_str() != "2.0" {
        return Err(error::Error::InvalidVersion);
    }

    // check method is not reserved (ie: starts with "rpc.")
    if request.method.starts_with("rpc.") {
        return Err(error::Error::ReservedMethodPrefix);
    }

    // call the method handler
    let id = request.id.clone();
    let response = match process(request, state) {
        Ok(response) => response,
        Err(err) => {
            tracing::error!("Error processing request: {}", err);
            Response::error(
                id,
                -32603,
                "Internal error".into(),
                Some(err.to_string().into()), // todo: dont expose this by default, config
            )
        }
    };

    Ok(response)
}

fn send_response(request: tiny_http::Request, response: Response) -> Result<()> {
    let data = serde_json::to_string(&response)?;
    let response = HttpResponse::from_string(data);
    Ok(request.respond(response)?)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Option<Id>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn result(id: Option<Id>, value: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(value),
            error: None,
        }
    }

    pub fn error(id: Option<Id>, code: i64, message: String, data: Option<Value>) -> Self {
        let err = RpcError {
            code,
            message,
            data,
        };
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(err),
        }
    }

    pub fn from_error(id: Option<Id>, error: error::Error) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error.as_rpc_error()),
        }
    }

    pub fn unimplemented(id: Option<Id>) -> Self {
        Self::error(id, METHOD_NOT_FOUND, "Method not found.".into(), None)
    }

    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn is_result(&self) -> bool {
        self.result.is_some()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Id {
    Number(u64),
    String(String),
}

#[cfg(test)]
mod test {
    use super::*;
    use jsonrpc::Client;
    use serde_json::{json, value::to_raw_value};
    use tiny_http::Server;

    fn process(request: Request, _state: Arc<Mutex<()>>) -> Result<Response> {
        let response = match request.method.as_str() {
            "echo" => Response {
                jsonrpc: request.jsonrpc,
                id: request.id,
                result: request.params,
                error: None,
            },
            _ => unimplemented!(),
        };
        Ok(response)
    }

    #[test]
    fn echo() {
        let addr = "127.0.0.1:0";
        let server = Server::http(addr).unwrap();
        let state = Arc::new(Mutex::new(()));
        let rpc = JsonRpcServer::new(server, state, process);
        let port = rpc.port().unwrap();
        let url = format!("127.0.0.1:{}", port);

        let client = Client::simple_http(&url, None, None).unwrap();
        let val = "The Times 03/Jan/2009 Chancellor on brink of second bailout for banks";
        let raw = to_raw_value(val).unwrap();
        let params = &[raw.clone()];
        let request = client.build_request("echo", params);
        let req = request.clone();

        let response = client.send_request(request).unwrap();

        assert_eq!(response.id, req.id);
        assert_eq!(response.jsonrpc.unwrap().as_str(), req.jsonrpc.unwrap());
        let result = response.result.unwrap();
        let expected = serde_json::to_string(&json!([raw])).unwrap();
        assert_eq!(result.get(), expected.as_str());
    }

    #[test]
    fn rpc_dot_reserved() {
        let addr = "127.0.0.1:0";
        let server = Server::http(addr).unwrap();
        let state = Arc::new(Mutex::new(()));
        let rpc = JsonRpcServer::new(server, state, process);
        let port = rpc.port().unwrap();
        let url = format!("127.0.0.1:{}", port);

        let client = Client::simple_http(&url, None, None).unwrap();
        let request = client.build_request("rpc.reserved", &[]);

        let response = client.send_request(request).unwrap();
        // dbg!(&response.error);
        assert!(response.error.is_some());
    }

    #[test]
    fn response_serialization() {
        // result response must not include error key
        let response = Response {
            jsonrpc: "2.0".into(),
            id: Some(Id::Number(123)),
            result: Some(Value::Bool(true)),
            error: None,
        };
        let actual = serde_json::to_value(response).unwrap();
        let expected = json!({
            "jsonrpc": "2.0",
            "result": true,
            "id": 123,
        });
        assert_eq!(actual, expected);
        assert!(actual.get("error").is_none());

        // error response must not include result key
        let response = Response {
            jsonrpc: "2.0".into(),
            id: Some(Id::Number(123)),
            result: None,
            error: Some(RpcError {
                code: -32_000,
                message: "Sunlifter".into(),
                data: None,
            }),
        };
        let actual = serde_json::to_value(response).unwrap();
        let expected = json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32000,
                "message": "Sunlifter",
                "data": null,
            },
            "id": 123,
        });
        assert_eq!(actual, expected);
        assert!(actual.get("result").is_none());
    }
}
