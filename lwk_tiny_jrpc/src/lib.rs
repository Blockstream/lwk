#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use std::{
    fmt::Display,
    fs::File,
    io::{ErrorKind, Read},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub use config::Config;
pub use error::Error;
use error::{AsRpcError, InnerError, METHOD_NOT_FOUND};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use tiny_http::Server;
use tiny_http::{Header, Response as HttpResponse};

pub mod config;
pub mod error;

// re-export
pub use tiny_http;

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
                        tiny_http::Method::Get => {
                            // respond to the http GET request
                            let Some(mut path) = config.serve_dir.clone() else {
                                let message = "No serve_dir defined in server config.";
                                let response =
                                    HttpResponse::from_string(message).with_status_code(500);
                                send_http_response(http_request, response, message);
                                continue;
                            };
                            // remove starting slash
                            let file_name = http_request
                                .url()
                                .strip_prefix('/')
                                .expect("url starts with slash");
                            path.push(file_name);
                            // add index.html to directories
                            if path.is_dir() {
                                path.push("index.html");
                            }
                            match File::open(path) {
                                Ok(mut file) => {
                                    let mut buf = Vec::new();
                                    match file.read_to_end(&mut buf) {
                                        Ok(n) => log::trace!("GET: read {n} bytes"),
                                        Err(e) => {
                                            let message = "500: Internal error";
                                            let response = HttpResponse::from_string(message)
                                                .with_status_code(500);
                                            send_http_response(
                                                http_request,
                                                response,
                                                format!("{message}: {e}").as_str(),
                                            );
                                            continue;
                                        }
                                    }
                                    // todo: content-type headers, this is non-trivial and not strictly necessary right now
                                    let response = HttpResponse::from_data(buf);
                                    let message = "File for GET request";
                                    send_http_response(http_request, response, message);
                                }
                                Err(e) if matches!(e.kind(), ErrorKind::NotFound) => {
                                    // 404
                                    let message = "404: File not found";
                                    let response =
                                        HttpResponse::from_string(message).with_status_code(404);
                                    send_http_response(http_request, response, message);
                                }
                                Err(e) => {
                                    // 500
                                    let message = "500: Internal error";
                                    let response =
                                        HttpResponse::from_string(message).with_status_code(500);
                                    send_http_response(
                                        http_request,
                                        response,
                                        format!("{message}: {e}").as_str(),
                                    );
                                }
                            }
                        }
                        tiny_http::Method::Options => {
                            // respond to the http OPTIONS request, normally for CORS
                            let allow = Header::from_str("Allow: GET, POST, OPTIONS")
                                .expect("valid header");
                            let mut response = HttpResponse::empty(204).with_header(allow);
                            for header in config.headers.clone().into_iter() {
                                response.add_header(header);
                            }
                            let message = "OPTIONS request";
                            send_http_response(http_request, response, message);
                        }
                        tiny_http::Method::Post => {
                            // validate/parse the jsonrpc POST request
                            let response = match validate_jsonrpc_request(&mut http_request) {
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
                            if let Err(err) =
                                send_jsonrpc_response(http_request, response, &config.headers)
                            {
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

fn validate_jsonrpc_request(http_request: &mut tiny_http::Request) -> Result<Request, InnerError> {
    log::debug!(
        "received request - method: {:?}, url: {:?}, headers: {:?}",
        http_request.method(),
        http_request.url(),
        http_request.headers()
    );

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

    // parse json into request
    let mut s = String::new(); // todo: performance
    http_request.as_reader().read_to_string(&mut s)?;

    let request: Request = serde_json::from_str(&s)?;

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
    headers: &[Header],
) -> Result<(), InnerError> {
    let data = serde_json::to_string(&response)?;
    let mut response = HttpResponse::from_string(data);
    for header in headers.iter() {
        response.add_header(header.clone());
    }
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
    use std::{
        fs::File,
        io::{Read, Write},
        net::TcpStream,
        path::PathBuf,
    };

    use super::*;
    use jsonrpc::Client;
    use serde_json::{json, value::to_raw_value};
    use tiny_http::Server;

    fn process(request: Request, _state: Arc<Mutex<()>>) -> Result<Response, Error> {
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

    fn send_http_request(stream: &mut TcpStream, request: &str) -> Vec<u8> {
        // Add Connection: close header to all requests
        let request = request.trim_end_matches("\r\n\r\n");
        let request = format!("{request}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        response
    }

    fn assert_response_contains(response: &[u8], expected: &str) {
        assert!(String::from_utf8_lossy(response).contains(expected));
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
    fn rpc_dot_reserved() {
        let addr = "127.0.0.1:0";
        let server = Server::http(addr).unwrap();
        let state = Arc::new(Mutex::new(()));
        let rpc = JsonRpcServer::new(server, Config::default(), state, process);
        let port = rpc.port().unwrap();
        let url = format!("127.0.0.1:{port}");

        let client = Client::simple_http(&url, None, None).unwrap();
        let request = client.build_request("rpc.reserved", None);

        let response = client.send_request(request).unwrap();
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

    #[test]
    fn http_options() {
        let addr = "127.0.0.1:0";
        let server = Server::http(addr).unwrap();
        let state = Arc::new(Mutex::new(()));
        let config = Config {
            headers: vec![
                Header::from_str("Access-Control-Allow-Origin: http://127.0.0.1:8000").unwrap(),
                Header::from_str("Access-Control-Allow-Headers: content-type").unwrap(),
            ],
            ..Default::default()
        };
        let rpc = JsonRpcServer::new(server, config, state, process);
        let port = rpc.port().unwrap();

        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        let request = "OPTIONS / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        let response = send_http_request(&mut stream, request);

        assert_response_contains(&response, "HTTP/1.1 204");
        assert_response_contains(&response, "Allow: GET, POST, OPTIONS");
        assert_response_contains(
            &response,
            "Access-Control-Allow-Origin: http://127.0.0.1:8000",
        );
        assert_response_contains(&response, "Access-Control-Allow-Headers: content-type");
    }

    fn make_file(dir_path: PathBuf, file_name: String, data: &[u8]) -> File {
        let mut path = dir_path;
        path.push(file_name);
        let mut file = File::create(path).unwrap();
        file.write_all(data).unwrap();
        file
    }

    #[test]
    fn http_get() {
        let addr = "127.0.0.1:0";
        let server = Server::http(addr).unwrap();
        let state = Arc::new(Mutex::new(()));

        // create the http serve dir
        let dir = tempfile::tempdir().unwrap();

        let dir_path = dir.keep();

        let config = Config {
            serve_dir: Some(dir_path.clone()),
            ..Default::default()
        };
        let rpc = JsonRpcServer::new(server, config, state, process);
        let port = rpc.port().unwrap();

        // create files to GET
        let file_types = [
            ("html", "<!doctype html>".as_bytes()),
            ("css", include_bytes!("../test/data/file.css")),
            ("js", include_bytes!("../test/data/file.js")),
            ("ico", include_bytes!("../test/data/file.ico")),
            ("jpg", include_bytes!("../test/data/file.jpg")),
            ("png", include_bytes!("../test/data/file.png")),
            ("svg", include_bytes!("../test/data/file.svg")),
        ];

        for (ext, data) in file_types.into_iter() {
            let file_name = format!("file.{ext}");
            make_file(dir_path.clone(), file_name.clone(), data);

            let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
            let request = format!("GET /{file_name} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
            let response = send_http_request(&mut stream, &request);

            assert_response_contains(&response, "HTTP/1.1 200");
            // Find the body after the headers
            if let Some(body_start) = response.windows(4).position(|window| window == b"\r\n\r\n") {
                let body = &response[body_start + 4..];
                assert_eq!(body, data);
            } else {
                panic!("No body found in response");
            }
        }

        // 404
        let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        let request = "GET /missing.file HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        let response = send_http_request(&mut stream, request);

        assert_response_contains(&response, "HTTP/1.1 404");
        assert_response_contains(&response, "404: File not found");
    }
}
