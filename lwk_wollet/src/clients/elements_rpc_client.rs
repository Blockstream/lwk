use crate::Error;
use crate::{WalletTxOut, WolletDescriptor};

use bitcoincore_rpc::{Auth, Client, RpcApi};

/// A client to issue RPCs to a Elements node
pub struct ElementsRpcClient {
    inner: Client,
}

impl ElementsRpcClient {
    /// Create a new Elements RPC client
    pub fn new_from_credentials(url: &str, user: &str, pass: &str) -> Result<Self, Error> {
        let auth = Auth::UserPass(user.to_string(), pass.to_string());
        let inner = Client::new(url, auth)?;
        Ok(Self { inner })
    }

    /// Get the blockchain height
    pub fn height(&self) -> Result<u64, Error> {
        self.inner
            .call::<serde_json::Value>("getblockcount", &[])?
            .as_u64()
            .ok_or_else(|| Error::ElementsRpcUnexpectedReturn("getblockcount".into()))
    }

    /// Get the wallet utxos
    pub fn utxos(&self, _desc: &WolletDescriptor, _start: u32, _end: u32) -> Result<Vec<WalletTxOut>, Error> {
        // TODO: handle multipath
        // TODO: WolletDescriptor::to_single_descriptors()
        // TODO: convert to "bitcoin" descriptor
        // call scantxoutset
        // get transactions
        // TODO: unblind 
        // super::try_unblind
        //
        // txid
        // vout
        // scriptPubKey
        // desc
        // height
        todo!();
    }
}
