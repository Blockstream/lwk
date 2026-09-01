use elements::{Transaction, Txid};

use super::asyncr;
#[cfg(not(target_arch = "wasm32"))]
use super::{blocking, blocking::BlockchainBackend};
use crate::RegistryError;

#[cfg(not(target_arch = "wasm32"))]
use lwk_registry::TxFetcher;
use lwk_registry::TxFetcherAsync;

#[cfg(feature = "esplora")]
impl TxFetcherAsync for asyncr::EsploraClient {
    async fn get_transaction(&self, txid: Txid) -> Result<Transaction, RegistryError> {
        asyncr::EsploraClient::get_transaction(self, txid)
            .await
            .map_err(|e| lwk_registry::error::Error::Generic(e.to_string()))
    }
}

#[cfg(feature = "esplora")]
impl TxFetcherAsync for asyncr::WaterfallsClient {
    async fn get_transaction(&self, txid: Txid) -> Result<Transaction, RegistryError> {
        asyncr::WaterfallsClient::get_transaction(self, txid)
            .await
            .map_err(|e| lwk_registry::error::Error::Generic(e.to_string()))
    }
}

#[cfg(all(feature = "esplora", not(target_arch = "wasm32")))]
impl TxFetcher for blocking::EsploraClient {
    fn get_transaction(&self, txid: Txid) -> Result<Transaction, RegistryError> {
        BlockchainBackend::get_transaction(self, txid)
            .map_err(|e| lwk_registry::error::Error::Generic(e.to_string()))
    }
}

#[cfg(all(feature = "esplora", not(target_arch = "wasm32")))]
impl TxFetcher for blocking::WaterfallsClient {
    fn get_transaction(&self, txid: Txid) -> Result<Transaction, RegistryError> {
        BlockchainBackend::get_transaction(self, txid)
            .map_err(|e| lwk_registry::error::Error::Generic(e.to_string()))
    }
}
#[cfg(all(feature = "electrum", not(target_arch = "wasm32")))]
impl TxFetcher for blocking::electrum_client::ElectrumClient {
    fn get_transaction(&self, txid: Txid) -> Result<Transaction, RegistryError> {
        BlockchainBackend::get_transaction(self, txid)
            .map_err(|e| lwk_registry::error::Error::Generic(e.to_string()))
    }
}
