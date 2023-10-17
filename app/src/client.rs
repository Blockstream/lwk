use std::net::SocketAddr;

use crate::model::*;
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

    pub fn version(&self) -> Result<VersionResponse> {
        let request = self.client.build_request("version", &[]);
        let response = self.client.send_request(request)?;
        // todo: error
        Ok(serde_json::from_str(response.result.unwrap().get()).unwrap())
    }

    pub fn generate_signer(&self) -> Result<SignerGenerateResponse> {
        let request = self.client.build_request("generate_signer", &[]);
        let response = self.client.send_request(request)?;
        // todo: error
        Ok(serde_json::from_str(response.result.unwrap().get()).unwrap())
    }
}
