#[cfg(feature = "blocking")]
mod electrum;
mod esplora;

use std::sync::Arc;

#[cfg(feature = "blocking")]
pub use electrum::ElectrumClient;
pub use esplora::EsploraClient;

use async_trait::async_trait;
use boltz_client::{elements, error::Error, network::LiquidChain};

pub enum AnyClient {
    #[cfg(feature = "blocking")]
    Electrum(Arc<ElectrumClient>),
    Esplora(Arc<EsploraClient>),
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl boltz_client::network::LiquidClient for AnyClient {
    async fn get_address_utxo(
        &self,
        address: &elements::Address,
    ) -> Result<Option<(elements::OutPoint, elements::TxOut)>, Error> {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.get_address_utxo(address).await,
            AnyClient::Esplora(client) => client.get_address_utxo(address).await,
        }
    }

    async fn get_genesis_hash(&self) -> Result<elements::BlockHash, Error> {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.get_genesis_hash().await,
            AnyClient::Esplora(client) => client.get_genesis_hash().await,
        }
    }

    async fn broadcast_tx(&self, signed_tx: &elements::Transaction) -> Result<String, Error> {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.broadcast_tx(signed_tx).await,
            AnyClient::Esplora(client) => client.broadcast_tx(signed_tx).await,
        }
    }

    fn network(&self) -> LiquidChain {
        match self {
            #[cfg(feature = "blocking")]
            AnyClient::Electrum(client) => client.network(),
            AnyClient::Esplora(client) => client.network(),
        }
    }
}
