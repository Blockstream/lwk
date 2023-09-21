use std::{
    result,
    sync::Arc,
    thread::{self, JoinHandle},
};

use serde_derive::{Deserialize, Serialize};
use tiny_http::Response as HttpResponse;
use tiny_http::Server;

pub mod error;

pub type Result<T> = result::Result<T, error::Error>;

pub struct JsonRpcServer {
    server: Arc<Server>,
}

impl JsonRpcServer {
    ///
    pub fn new(server: Server) -> Self {
        Self {
            server: Arc::new(server),
        }
    }

    ///
    pub fn port(&self) -> Option<u16> {
        self.server.server_addr().to_ip().map(|addr| addr.port())
    }

    ///
    pub fn run<F>(&self, func: F) -> JoinHandle<Result<()>>
    where
        F: Fn(Request) -> Result<Response> + Send + Sync + 'static,
    {
        let server = self.server.clone();

        // todo: multiple worker threads
        thread::spawn(move || -> Result<()> {
            loop {
                let mut server_req = server.recv()?;

                // println!(
                //     "received request! method: {:?}, url: {:?}, headers: {:?}",
                //     server_req.method(),
                //     server_req.url(),
                //     server_req.headers()
                // );

                // todo: check content type is application/json

                // todo: check method is not reserved (ie: starts with "rpc.")

                // parse json into request
                let mut s = String::new(); // todo: this could be more performant - profile
                server_req.as_reader().read_to_string(&mut s)?;
                let request: Request = serde_json::from_str(&s).unwrap();
                dbg!((&request.id, &request.params, &request.jsonrpc)); // temporarily satisfy clippy
                dbg!(&request.method);

                // call method handler
                let response = func(request)?; // todo error variant

                // send response
                let data = serde_json::to_string(&response)?;
                let http = HttpResponse::from_string(data);
                server_req.respond(http)?;
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Request {
    jsonrpc: String,
    id: Option<Id>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Response {
    jsonrpc: String,
    id: Option<Id>,
    result: Option<serde_json::Value>,
    error: Option<RpcError>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RpcError {
    code: i64,
    message: String,
    data: Option<serde_json::Value>,
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
    use serde_json::value::to_raw_value;
    use tiny_http::Server;

    fn process(request: Request) -> Result<Response> {
        dbg!(&request);
        let response = match request.method.as_str() {
            "echo" => Response {
                jsonrpc: request.jsonrpc,
                id: request.id,
                result: request.params,
                error: None,
            },
            _ => todo!(),
        };
        Ok(response)
    }

    #[test]
    fn echo() {
        let addr = "127.0.0.1:0";
        let server = Server::http(addr).unwrap();
        let rpc = JsonRpcServer::new(server);
        let port = rpc.port().unwrap();
        dbg!(&port);
        let url = format!("127.0.0.1:{}", port);
        dbg!(&url);
        let _handle = rpc.run(process);

        let client = Client::simple_http(&url, None, None).unwrap();
        let val = "The Times 03/Jan/2009 Chancellor on brink of second bailout for banks";
        let params = &[to_raw_value(val).unwrap()];
        let req = client.build_request("echo", params);
        dbg!(&req);
        let req_clone = req.clone();
        let response = client.send_request(req).unwrap();
        dbg!(&response);
        assert_eq!(response.id, req_clone.id);
        assert_eq!(
            response.jsonrpc.unwrap().as_str(),
            req_clone.jsonrpc.unwrap()
        );
        // assert_eq!(
        //     response.result.unwrap().to_string(),
        //     req_clone.params[0].to_string()
        // );
    }
}
