use std::net::SocketAddr;

use crate::Result;

pub struct Client {
    client: jsonrpc::Client,
}

impl Client {
    pub fn new(addr: SocketAddr) -> Result<Self> {
        let url = addr.to_string();
        let client = jsonrpc::Client::simple_http(&url, None, None)?;
        Ok(Self { client })
    }

    pub fn version(&self) -> Result<String> {
        let request = self.client.build_request("version", &[]);
        let response = self.client.send_request(request)?;
        // todo: error
        let result = response.result.unwrap().to_string();
        let version: String = serde_json::from_str(&result).unwrap();
        Ok(version)
    }
}
