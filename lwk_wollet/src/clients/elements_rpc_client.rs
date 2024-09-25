use crate::ElementsNetwork;
use crate::Error;

use bitcoincore_rpc::{Auth, Client, RpcApi};

/// A client to issue RPCs to a Elements node
pub struct ElementsRpcClient {
    inner: Client,
    #[allow(unused)]
    network: ElementsNetwork,
}

impl ElementsRpcClient {
    /// Create a new Elements RPC client
    pub fn new(network: ElementsNetwork, url: &str, auth: Auth) -> Result<Self, Error> {
        let inner = Client::new(url, auth)?;
        Ok(Self { inner, network })
    }

    /// Create a new Elements RPC client from credentials
    pub fn new_from_credentials(
        network: ElementsNetwork,
        url: &str,
        user: &str,
        pass: &str,
    ) -> Result<Self, Error> {
        let auth = Auth::UserPass(user.to_string(), pass.to_string());
        Self::new(network, url, auth)
    }

    /// Get the blockchain height
    pub fn height(&self) -> Result<u64, Error> {
        self.inner
            .call::<serde_json::Value>("getblockcount", &[])?
            .as_u64()
            .ok_or_else(|| Error::ElementsRpcUnexpectedReturn("getblockcount".into()))
    }
}
