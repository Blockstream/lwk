use std::net::SocketAddr;

use serde::de::DeserializeOwned;
use serde_json::value::to_raw_value;
use serde_json::Value;

use crate::error::Error;
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
        result_or_error(response)
    }

    pub fn generate_signer(&self) -> Result<GenerateSignerResponse> {
        let request = self.client.build_request("generate_signer", None);
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn load_signer(&self, mnemonic: String, name: String) -> Result<SignerResponse> {
        let params = to_raw_value(&LoadSignerRequest { mnemonic, name })?;
        let request = self.client.build_request("load_signer", Some(&params));
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn list_wallets(&self) -> Result<ListWalletsResponse> {
        let request = self.client.build_request("list_wallets", None);
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn load_wallet(&self, descriptor: String, name: String) -> Result<WalletResponse> {
        let params = to_raw_value(&LoadWalletRequest { descriptor, name })?;
        let request = self.client.build_request("load_wallet", Some(&params));
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn unload_wallet(&self, name: String) -> Result<UnloadWalletResponse> {
        let params = to_raw_value(&UnloadWalletRequest { name })?;
        let request = self.client.build_request("unload_wallet", Some(&params));
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn unload_signer(&self, name: String) -> Result<UnloadSignerResponse> {
        let params = to_raw_value(&UnloadSignerRequest { name })?;
        let request = self.client.build_request("unload_signer", Some(&params));
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn list_signers(&self) -> Result<ListSignersResponse> {
        let request = self.client.build_request("list_signers", None);
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn balance(&self, name: String) -> Result<BalanceResponse> {
        let params = to_raw_value(&BalanceRequest { name })?;
        let request = self.client.build_request("balance", Some(&params));
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn address(&self, name: String, index: Option<u32>) -> Result<AddressResponse> {
        let params = to_raw_value(&AddressRequest { name, index })?;
        let request = self.client.build_request("address", Some(&params));
        let response = self.client.send_request(request)?;
        result_or_error(response)
    }

    pub fn stop(&self) -> Result<Value> {
        let request = self.client.build_request("stop", None);
        let _response = self.client.send_request(request)?;
        Ok(Value::Null)
    }
}

fn result_or_error<T: DeserializeOwned>(
    response: jsonrpc::Response,
) -> std::result::Result<T, Error> {
    match response.result.as_ref() {
        Some(result) => Ok(serde_json::from_str(result.get()).unwrap()),
        None => match response.error {
            Some(rpc_err) => Err(Error::RpcError(rpc_err)),
            None => Err(Error::NeitherResultNorErrorSet),
        },
    }
}
