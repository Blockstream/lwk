use std::collections::{HashMap, HashSet};

use age::x25519::Recipient;
use elements::{BlockHash, Script, Txid};
use tokio::runtime::Runtime;

use crate::{
    cache::Height,
    clients::{asyncr, Capability, Data, History, WaterfallsClientBuilder},
    wollet::WolletState,
    Error, Network, WolletDescriptor,
};

use super::BlockchainBackend;

impl WaterfallsClientBuilder {
    /// Build a blocking Waterfalls client.
    pub fn build_blocking(self) -> Result<WaterfallsClient, Error> {
        Ok(WaterfallsClient {
            rt: Runtime::new()?,
            client: WaterfallsClientBuilder::build(self)?,
        })
    }
}

#[derive(Debug)]
/// A blockchain backend implementation based on the
/// [Waterfalls HTTP API](https://github.com/RCasatta/waterfalls).
///
/// Waterfalls is Esplora-compatible for common chain operations and adds
/// descriptor-based wallet scan endpoints.
pub struct WaterfallsClient {
    rt: Runtime,
    client: asyncr::WaterfallsClient,
}

impl WaterfallsClient {
    /// Create a new Waterfalls client.
    pub fn new(url: &str, network: Network) -> Result<Self, Error> {
        Ok(Self {
            rt: Runtime::new()?,
            client: asyncr::WaterfallsClient::new(network, url),
        })
    }

    /// Do not encrypt the descriptor when using the Waterfalls endpoint.
    pub fn avoid_encryption(&mut self) {
        self.client.avoid_encryption();
    }

    /// Returns the Waterfalls server recipient key using a cached value or by asking the server its key.
    pub fn waterfalls_server_recipient(&mut self) -> Result<Recipient, Error> {
        self.rt.block_on(self.client.waterfalls_server_recipient())
    }

    /// Set the Waterfalls server recipient key.
    ///
    /// This is used to encrypt the descriptor when calling the Waterfalls endpoint.
    pub fn set_waterfalls_server_recipient(&mut self, recipient: Recipient) {
        self.client.set_waterfalls_server_recipient(recipient);
    }

    /// Return the descriptor string to use with Waterfalls descriptor endpoints.
    pub fn waterfalls_descriptor(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<String, Error> {
        self.rt
            .block_on(self.client.waterfalls_descriptor(descriptor))
    }

    /// Query the last used derivation index for a descriptor from the Waterfalls server.
    pub fn last_used_index(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<asyncr::LastUsedIndexResponse, Error> {
        self.rt.block_on(self.client.last_used_index(descriptor))
    }
}

impl BlockchainBackend for WaterfallsClient {
    fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        self.rt.block_on(self.client.tip())
    }

    fn broadcast(&self, tx: &elements::Transaction) -> Result<elements::Txid, crate::Error> {
        self.rt.block_on(self.client.broadcast(tx))
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<elements::Transaction>, Error> {
        self.rt.block_on(self.client.get_transactions(txids))
    }

    fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        self.rt
            .block_on(self.client.get_headers(heights, height_blockhash))
    }

    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        self.rt.block_on(self.client.get_scripts_history(scripts))
    }

    fn capabilities(&self) -> HashSet<Capability> {
        self.client.capabilities()
    }

    fn get_history_waterfalls<S: WolletState>(
        &mut self,
        descriptor: &WolletDescriptor,
        state: &S,
        to_index: u32,
    ) -> Result<Data, Error> {
        self.rt.block_on(
            self.client
                .get_history_waterfalls(descriptor, state, to_index),
        )
    }

    fn utxo_only(&self) -> bool {
        self.client.utxo_only()
    }
}
