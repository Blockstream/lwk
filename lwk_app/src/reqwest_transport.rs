use std::{fmt, time::Duration};

use jsonrpc::{Request, Response};

#[derive(Clone, Debug)]
pub struct ReqwestHttpTransport {
    /// URL of the RPC server.
    url: String,
    /// timeout only supports second granularity.
    timeout: Duration,
}

impl ReqwestHttpTransport {
    pub fn new(url: String, timeout: Duration) -> Self {
        ReqwestHttpTransport { url, timeout }
    }
    fn request<R>(&self, req: impl serde::Serialize) -> Result<R, crate::Error>
    where
        R: for<'a> serde::de::Deserialize<'a>,
    {
        let client = reqwest::blocking::ClientBuilder::new()
            .timeout(self.timeout)
            .build()?;
        let response = client.post(&self.url).json(&req).send()?;
        Ok(response.json()?)
    }
}

impl From<crate::Error> for jsonrpc::Error {
    fn from(value: crate::Error) -> Self {
        match value {
            crate::Error::JsonRpcClient(e) => e,
            e => jsonrpc::Error::Transport(Box::new(e)),
        }
    }
}

impl jsonrpc::Transport for ReqwestHttpTransport {
    fn send_request(&self, req: Request) -> Result<Response, jsonrpc::Error> {
        Ok(self.request(req)?)
    }

    fn send_batch(&self, reqs: &[Request]) -> Result<Vec<Response>, jsonrpc::Error> {
        Ok(self.request(reqs)?)
    }

    fn fmt_target(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.url)
    }
}
