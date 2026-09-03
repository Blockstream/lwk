#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use std::{
    fmt::Display,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub use config::Config;
pub use error::Error;
use error::{AsRpcError, InnerError, INTERNAL_ERROR, INVALID_PARAMS, METHOD_NOT_FOUND};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use tiny_http::Response as HttpResponse;
use tiny_http::Server;

pub mod config;
pub mod error;

// re-export
pub use tiny_http;

/// Maximum accepted size, in bytes, of a JSON-RPC request body.
///
/// Bounds worst-case memory use per request; requests over this size are rejected with a 413
/// before (or, if the `Content-Length` header is absent or understated, while) the body is read.
const MAX_REQUEST_BODY_BYTES: usize = 16 * 1024 * 1024;

pub struct JsonRpcServer {
    server: Arc<Server>,
    handles: Vec<JoinHandle<Result<(), Error>>>,
    running: Arc<AtomicBool>,
    config: Config,
}

impl JsonRpcServer {
    /// Creates and runs a new JSON RPC Server.
    pub fn new<F, T>(server: Server, config: Config, state: Arc<Mutex<T>>, func: F) -> Self
    where
        F: Fn(Request, Arc<Mutex<T>>) -> Result<Response, Error> + Clone + Send + Sync + 'static,
        T: Send + 'static,
    {
        Self::run(Arc::new(server), config, state, func)
    }

    /// Returns a reference to the [`tiny_http::ListenAddr`] of the server.
    pub fn server_addr(&self) -> tiny_http::ListenAddr {
        self.server.server_addr()
    }

    /// Returns the IP port unless the underlying tiny_http server is listening on a Unix socket.
    pub fn port(&self) -> Option<u16> {
        self.server.server_addr().to_ip().map(|addr| addr.port())
    }

    /// Returns a reference to the [`Config`] used when creating the JSON RPC Server.
    pub fn config(&self) -> &Config {
        &self.config
    }

    fn run<F, T>(server: Arc<Server>, config: Config, state: Arc<Mutex<T>>, func: F) -> Self
    where
        F: Fn(Request, Arc<Mutex<T>>) -> Result<Response, Error> + Clone + Send + Sync + 'static,
        T: Send + 'static,
    {
        let mut handles = Vec::with_capacity(4);
        let running = Arc::new(AtomicBool::new(true));

        for _ in 0..config.num_threads.get() {
            let server = server.clone();
            let func = func.clone();
            let state = state.clone();
            let running = running.clone();
            let config = config.clone();
            let handle = thread::spawn(move || {
                loop {
                    // receive http request
                    let mut http_request = match server.recv_timeout(Duration::from_millis(100)) {
                        Ok(Some(request)) => request,
                        Ok(None) => {
                            // timeout, checks we aren't stopped
                            if running.load(Ordering::SeqCst) {
                                continue;
                            } else {
                                break;
                            }
                        }
                        Err(err) => {
                            // not much to do if recv fails
                            log::error!("recv error: {err}");
                            continue;
                        }
                    };

                    // check request method
                    match http_request.method() {
                        tiny_http::Method::Post => {
                            // validate/parse the jsonrpc POST request
                            let result = validate_jsonrpc_request(&mut http_request, &config);
                            if let Err(InnerError::PayloadTooLarge) = result {
                                let message = format!(
                                    "413: Payload too large (max {MAX_REQUEST_BODY_BYTES} bytes)"
                                );
                                let response =
                                    HttpResponse::from_string(&message).with_status_code(413);
                                send_http_response(http_request, response, &message);
                                continue;
                            }
                            let response = match result {
                                Ok(request) => {
                                    // handle the request
                                    let id = request.id.clone();
                                    match handle_jsonrpc_request(
                                        request,
                                        state.clone(),
                                        func.clone(),
                                    ) {
                                        Ok(response) => response,
                                        Err(Error::Stop) => {
                                            running.store(false, Ordering::SeqCst);
                                            Response::from_error(id, Error::Stop)
                                        }
                                        Err(err) => Response::from_error(id, err),
                                    }
                                }
                                Err(err) => {
                                    // no id since we couldn't validate the request...
                                    Response::from_error(None, err)
                                }
                            };

                            // send the response
                            if let Err(err) = send_jsonrpc_response(http_request, response) {
                                log::error!("send_response error: {err}");
                            }
                        }
                        other => {
                            let message =
                                format!("500: Internal error - method {other} not implemented.");
                            let response =
                                HttpResponse::from_string(&message).with_status_code(500);
                            send_http_response(http_request, response, &message);
                        }
                    }
                }
                Ok(())
            });
            handles.push(handle);
        }

        Self {
            server,
            handles,
            running,
            config,
        }
    }

    /// Stops the server.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Returns true unless the server has been stopped.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Waits for the server threads to finish by calling `join` on each associated [`JoinHandle`].
    pub fn join_threads(&mut self) {
        while let Some(handle) = self.handles.pop() {
            let _ = handle.join();
        }
    }
}

// sends the response and debug logs the status code and message, or logs the error.
fn send_http_response<R>(http_request: tiny_http::Request, response: HttpResponse<R>, message: &str)
where
    R: Read,
{
    let status = response.status_code();
    match http_request.respond(response) {
        Ok(()) => log::debug!(
            "Sent response with status code: {status:?} and response message: {message}"
        ),
        Err(e) => log::error!("Error sending response: {e}"),
    }
}

/// Checks the `Authorization` header against `config.expected_auth_header`, if set,
/// using a constant-time comparison.
fn check_auth(http_request: &tiny_http::Request, config: &Config) -> Result<(), InnerError> {
    if let Some(expected) = &config.expected_auth_header {
        let provided = http_request
            .headers()
            .iter()
            .find(|h| {
                h.field
                    .as_str()
                    .as_str()
                    .eq_ignore_ascii_case("authorization")
            })
            .map(|h| h.value.as_str());
        let authorized = match provided {
            Some(provided) => {
                use subtle::ConstantTimeEq;
                bool::from(provided.as_bytes().ct_eq(expected.as_bytes()))
            }
            None => false,
        };
        if !authorized {
            return Err(InnerError::Unauthorized);
        }
    }
    Ok(())
}

fn validate_jsonrpc_request(
    http_request: &mut tiny_http::Request,
    config: &Config,
) -> Result<Request, InnerError> {
    // check authorization, if configured
    check_auth(http_request, config)?;

    // check content-type header exists
    let content_header = http_request
        .headers()
        .iter()
        .find(|h| {
            h.field
                .as_str()
                .as_str()
                .eq_ignore_ascii_case("content-type")
        })
        .ok_or(InnerError::NoContentType)?;

    // check content-type is application/json
    if !content_header
        .value
        .as_str()
        .trim()
        .to_ascii_lowercase()
        .contains("application/json")
    {
        return Err(InnerError::WrongContentType);
    }

    // reject requests whose advertised size already exceeds the limit
    if let Some(len) = http_request.body_length() {
        if len > MAX_REQUEST_BODY_BYTES {
            return Err(InnerError::PayloadTooLarge);
        }
    }

    // parse json into request
    let mut s = String::new(); // todo: performance

    // bound the actual bytes read too, in case Content-Length is missing or understated
    let mut limited_reader = http_request
        .as_reader()
        .take(MAX_REQUEST_BODY_BYTES as u64 + 1);
    limited_reader.read_to_string(&mut s)?;
    if s.len() > MAX_REQUEST_BODY_BYTES {
        return Err(InnerError::PayloadTooLarge);
    }

    // parse as generic JSON first, so a syntactically valid document that just isn't a
    // valid Request object is reported as InvalidRequest rather than as a parse error
    let value: Value = serde_json::from_str(&s)?;
    let request: Request = serde_json::from_value(value).map_err(|_| InnerError::InvalidRequest)?;

    Ok(request)
}

fn handle_jsonrpc_request<F, T>(
    request: Request,
    state: Arc<Mutex<T>>,
    process: F,
) -> Result<Response, Error>
where
    F: Fn(Request, Arc<Mutex<T>>) -> Result<Response, Error> + Clone + Send + Sync + 'static,
    T: Send + 'static,
{
    // check jsonrpc version
    if request.jsonrpc.as_str() != "2.0" {
        return Err(error::Error::Inner(InnerError::InvalidVersion));
    }

    // check method is not reserved (ie: starts with "rpc.")
    if request.method.starts_with("rpc.") {
        return Err(error::Error::Inner(InnerError::ReservedMethodPrefix));
    }

    // call the method handler
    let id = request.id.clone();
    let response = match process(request, state) {
        Ok(response) => response,
        Err(Error::Stop) => return Err(Error::Stop),
        Err(Error::Inner(err)) => {
            log::error!("Error processing request: {err}");
            Response::from_error(id, err)
        }
        Err(Error::Implementation(err)) => Response::from_error(id, err),
    };

    Ok(response)
}

fn send_jsonrpc_response(
    request: tiny_http::Request,
    response: Response,
) -> Result<(), InnerError> {
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

    pub(crate) fn from_error<E: AsRpcError>(id: Option<Id>, error: E) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error.as_rpc_error()),
        }
    }

    pub fn unimplemented(id: Option<Id>, message: String) -> Self {
        Self::error(id, METHOD_NOT_FOUND, message, None)
    }

    pub fn invalid_params(id: Option<Id>, message: String) -> Self {
        Self::error(id, INVALID_PARAMS, message, None)
    }

    pub fn internal_error(id: Option<Id>, message: String) -> Self {
        Self::error(id, INTERNAL_ERROR, message, None)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Id {
    Number(u64),
    String(String),
}

#[cfg(test)]
mod test {
    use std::{io::Write, net::TcpStream};

    use super::*;
    use jsonrpc::Client;
    use serde_json::{json, value::to_raw_value};
    use tiny_http::Server;

    fn process(request: Request, _state: Arc<Mutex<()>>) -> Result<Response, Error> {
        match request.method.as_str() {
            "echo" => Ok(Response {
                jsonrpc: request.jsonrpc,
                id: request.id,
                result: request.params,
                error: None,
            }),
            "stop" => Err(Error::Stop),
            _ => unimplemented!(),
        }
    }

    // sends a raw HTTP request over `stream` and returns the raw response bytes
    fn send_http_request(stream: &mut TcpStream, request: &str) -> Vec<u8> {
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    }

    // parses the JSON-RPC body out of a raw HTTP response
    fn parse_response(raw: &[u8]) -> Response {
        let body_start = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        serde_json::from_slice(&raw[body_start..]).unwrap()
    }

    #[test]
    fn echo() {
        let addr = "127.0.0.1:0";
        let server = Server::http(addr).unwrap();
        let state = Arc::new(Mutex::new(()));
        let mut rpc = JsonRpcServer::new(server, Config::default(), state, process);
        let port = rpc.port().unwrap();
        let url = format!("127.0.0.1:{port}");

        let client = Client::simple_http(&url, None, None).unwrap();
        let val = "The Times 03/Jan/2009 Chancellor on brink of second bailout for banks";
        let params = to_raw_value(val).unwrap();
        let request = client.build_request("echo", Some(&params));
        let req = request.clone();

        let response = client.send_request(request).unwrap();

        assert_eq!(response.id, req.id);
        assert_eq!(response.jsonrpc.unwrap().as_str(), req.jsonrpc.unwrap());
        let result = response.result.unwrap();
        let expected = serde_json::to_string(&json!(params)).unwrap();
        assert_eq!(result.get(), expected.as_str());

        rpc.stop();
        rpc.join_threads();
    }

    #[test]
    fn post_error_and_stop_paths() {
        let addr = "127.0.0.1:0";
        let server = Server::http(addr).unwrap();
        let state = Arc::new(Mutex::new(()));
        let token = "s3cr3t-token";
        let config = Config {
            expected_auth_header: Some(token.to_string()),
            ..Default::default()
        };
        let mut rpc = JsonRpcServer::new(server, config, state, process);
        let port = rpc.port().unwrap();
        let addr = format!("127.0.0.1:{port}");
        let auth = format!("Authorization: {token}\r\n");

        let post = |extra_headers: &str, body: &str| -> Response {
            let request = format!(
                "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let mut stream = TcpStream::connect(&addr).unwrap();
            let raw = send_http_request(&mut stream, &request);
            parse_response(&raw)
        };

        // reserved method prefix
        let response = post(
            &format!("{auth}Content-Type: application/json\r\n"),
            r#"{"jsonrpc":"2.0","id":1,"method":"rpc.reserved"}"#,
        );
        assert_eq!(response.error.unwrap().code, -32_003); // METHOD_RESERVED

        // invalid jsonrpc version
        let response = post(
            &format!("{auth}Content-Type: application/json\r\n"),
            r#"{"jsonrpc":"1.0","id":1,"method":"echo"}"#,
        );
        assert_eq!(response.error.unwrap().code, -32_004); // INVALID_VERSION

        // malformed JSON body
        let response = post(
            &format!("{auth}Content-Type: application/json\r\n"),
            "not json",
        );
        assert_eq!(response.error.unwrap().code, -32_700); // PARSE_ERROR

        // valid JSON, but not a valid Request object (missing "method")
        let response = post(
            &format!("{auth}Content-Type: application/json\r\n"),
            r#"{"jsonrpc":"2.0","id":1}"#,
        );
        assert_eq!(response.error.unwrap().code, -32_600); // INVALID_REQUEST

        // missing Content-Type
        let response = post(&auth, r#"{"jsonrpc":"2.0","id":1,"method":"echo"}"#);
        assert_eq!(response.error.unwrap().code, -32_001); // NO_CONTENT_TYPE

        // wrong Content-Type
        let response = post(
            &format!("{auth}Content-Type: text/plain\r\n"),
            r#"{"jsonrpc":"2.0","id":1,"method":"echo"}"#,
        );
        assert_eq!(response.error.unwrap().code, -32_002); // WRONG_CONTENT_TYPE

        // missing Authorization
        let response = post(
            "Content-Type: application/json\r\n",
            r#"{"jsonrpc":"2.0","id":1,"method":"echo"}"#,
        );
        assert_eq!(response.error.unwrap().code, -32_098); // UNAUTHORIZED

        // wrong Authorization
        let response = post(
            "Authorization: wrong\r\nContent-Type: application/json\r\n",
            r#"{"jsonrpc":"2.0","id":1,"method":"echo"}"#,
        );
        assert_eq!(response.error.unwrap().code, -32_098); // UNAUTHORIZED

        // oversized payload: the size check runs against the declared Content-Length
        // before the body is read, so the server responds before we finish sending it
        let big_len = MAX_REQUEST_BODY_BYTES + 1;
        let header = format!(
            "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\n{auth}Content-Type: application/json\r\nContent-Length: {big_len}\r\nConnection: close\r\n\r\n"
        );
        let mut stream = TcpStream::connect(&addr).unwrap();
        let _ = stream.write_all(header.as_bytes());
        let chunk = vec![b'x'; 64 * 1024];
        for _ in 0..(big_len / chunk.len() + 1) {
            if stream.write_all(&chunk).is_err() {
                break;
            }
        }
        let mut raw = Vec::new();
        std::io::Read::read_to_end(&mut stream, &mut raw).unwrap();
        assert!(String::from_utf8_lossy(&raw).starts_with("HTTP/1.1 413"));

        // valid authenticated request still works
        let response = post(
            &format!("{auth}Content-Type: application/json\r\n"),
            r#"{"jsonrpc":"2.0","id":1,"method":"echo","params":"hi"}"#,
        );
        assert_eq!(response.result.unwrap(), "hi");

        // a handler returning Error::Stop shuts the server down
        let response = post(
            &format!("{auth}Content-Type: application/json\r\n"),
            r#"{"jsonrpc":"2.0","id":1,"method":"stop"}"#,
        );
        assert_eq!(response.error.unwrap().code, -32_099); // STOP_ERROR
        assert!(!rpc.is_running());

        rpc.join_threads();
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

        // Response::error() and Response::unimplemented() build the same shape directly
        let err_response = Response::error(Some(Id::Number(1)), -32_000, "boom".into(), None);
        assert!(err_response.result.is_none());
        assert_eq!(err_response.error.unwrap().code, -32_000);

        let unimpl = Response::unimplemented(Some(Id::Number(1)), "nope".into());
        assert_eq!(unimpl.error.unwrap().code, METHOD_NOT_FOUND);

        let bad_params = Response::invalid_params(Some(Id::Number(1)), "".into());
        assert_eq!(bad_params.error.unwrap().code, INVALID_PARAMS);

        let internal = Response::internal_error(Some(Id::Number(1)), "".into());
        assert_eq!(internal.error.unwrap().code, INTERNAL_ERROR);
    }
}
