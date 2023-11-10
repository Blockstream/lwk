use std::net::SocketAddr;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::value::to_raw_value;
use serde_json::value::RawValue;
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

    fn make_request<Req, Res>(
        &self,
        method: &str,
        req: Option<Req>,
    ) -> std::result::Result<Res, Error>
    where
        Req: Serialize,
        Res: DeserializeOwned,
    {
        let params = req.map(|req| to_raw_value(&req)).transpose()?;
        let request = self.client.build_request(method, params.as_ref());
        tracing::trace!("---> {}", serde_json::to_string(&request).unwrap());
        let response = self.client.send_request(request)?;
        tracing::trace!("<--- {}", serde_json::to_string(&response).unwrap());
        match response.result.as_ref() {
            Some(result) => Ok(serde_json::from_str(result.get()).unwrap()),
            None => match response.error {
                Some(rpc_err) => Err(Error::RpcError(rpc_err)),
                None => Err(Error::NeitherResultNorErrorSet),
            },
        }
    }

    pub fn version(&self) -> Result<VersionResponse> {
        self.make_request("version", None::<Box<RawValue>>)
    }

    pub fn generate_signer(&self) -> Result<GenerateSignerResponse> {
        self.make_request("generate_signer", None::<Box<RawValue>>)
    }

    pub fn load_signer(&self, mnemonic: String, name: String) -> Result<SignerResponse> {
        let req = LoadSignerRequest { mnemonic, name };
        self.make_request("load_signer", Some(req))
    }

    pub fn list_wallets(&self) -> Result<ListWalletsResponse> {
        self.make_request("list_wallets", None::<Box<RawValue>>)
    }

    pub fn load_wallet(&self, descriptor: String, name: String) -> Result<WalletResponse> {
        let req = LoadWalletRequest { descriptor, name };
        self.make_request("load_wallet", Some(req))
    }

    pub fn unload_wallet(&self, name: String) -> Result<UnloadWalletResponse> {
        let req = UnloadWalletRequest { name };
        self.make_request("unload_wallet", Some(req))
    }

    pub fn unload_signer(&self, name: String) -> Result<UnloadSignerResponse> {
        let req = UnloadSignerRequest { name };
        self.make_request("unload_signer", Some(req))
    }

    pub fn list_signers(&self) -> Result<ListSignersResponse> {
        self.make_request("list_signers", None::<Box<RawValue>>)
    }

    pub fn balance(&self, name: String) -> Result<BalanceResponse> {
        let req = BalanceRequest { name };
        self.make_request("balance", Some(req))
    }

    pub fn address(&self, name: String, index: Option<u32>) -> Result<AddressResponse> {
        let req = AddressRequest { name, index };
        self.make_request("address", Some(req))
    }

    pub fn stop(&self) -> Result<Value> {
        // TODO discriminate only stop error
        let _: Result<Value> = self.make_request("stop", None::<Box<RawValue>>);
        Ok(Value::Null)
    }
}
