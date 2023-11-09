use std::net::SocketAddr;

use serde_json::value::to_raw_value;
use serde_json::Value;

use crate::model::*;
use crate::Result;

pub struct Client {
    client: jsonrpc::Client,
}

impl Client {
    pub fn new(addr: SocketAddr) -> Result<Self> {
        let url = addr.to_string();
        let client = jsonrpc::Client::simple_http(&url, None, None)?; // todo: auth
        Ok(Self { client })
    }

    pub fn version(&self) -> Result<VersionResponse> {
        let request = self.client.build_request("version", None);
        let response = self.client.send_request(request)?;
        // todo: error
        Ok(serde_json::from_str(response.result.unwrap().get()).unwrap())
    }

    pub fn generate_signer(&self) -> Result<GenerateSignerResponse> {
        let request = self.client.build_request("generate_signer", None);
        let response = self.client.send_request(request)?;
        // todo: error
        Ok(serde_json::from_str(response.result.unwrap().get()).unwrap())
    }

    pub fn load_signer(&self, mnemonic: String) -> Result<LoadSignerResponse> {
        let params = to_raw_value(&mnemonic)?;
        let request = self.client.build_request("load_signer", Some(&params));
        let response = self.client.send_request(request)?;
        // todo: error
        Ok(serde_json::from_str(response.result.unwrap().get()).unwrap())
    }

    pub fn load_wallet(&self, descriptor: String, name: String) -> Result<LoadWalletResponse> {
        let params = to_raw_value(&LoadWalletRequest { descriptor, name })?;
        let request = self.client.build_request("load_wallet", Some(&params));
        let response = self.client.send_request(request)?;
        // todo: error
        Ok(serde_json::from_str(response.result.unwrap().get()).unwrap())
    }

    pub fn balance(&self, name: String) -> Result<BalanceResponse> {
        let params = to_raw_value(&BalanceRequest { name })?;
        let request = self.client.build_request("balance", Some(&params));
        let response = self.client.send_request(request)?;
        // todo: error
        Ok(serde_json::from_str(response.result.unwrap().get()).unwrap())
    }

    pub fn stop(&self) -> Result<Value> {
        let request = self.client.build_request("stop", None);
        let _response = self.client.send_request(request)?;
        Ok(Value::Null)
    }
}
