use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use boltz_client::elements;
use boltz_client::error::Error;
use boltz_client::network::LiquidChain;
use boltz_client::ToHex;
use lwk_wollet::ElementsNetwork;

pub struct EsploraClient {
    inner: Arc<lwk_wollet::asyncr::EsploraClient>,
    network: ElementsNetwork,
}

impl EsploraClient {
    pub fn from_client(
        client: Arc<lwk_wollet::asyncr::EsploraClient>,
        network: ElementsNetwork,
    ) -> Self {
        Self {
            inner: client,
            network,
        }
    }

    pub fn new(url: &str, network: ElementsNetwork) -> Self {
        Self {
            inner: Arc::new(lwk_wollet::asyncr::EsploraClient::new(network, url)),
            network,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl boltz_client::network::LiquidClient for EsploraClient {
    async fn get_address_utxo(
        &self,
        address: &elements::Address,
    ) -> Result<Option<(elements::OutPoint, elements::TxOut)>, Error> {
        let spk = address.script_pubkey();
        let history = self
            .inner
            .get_scripts_history(&[&spk])
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        if history.is_empty() || history[0].is_empty() {
            return Ok(None);
        }

        let txids: Vec<_> = history[0].iter().map(|h| h.txid).collect();
        let txs = self
            .inner
            .get_transactions(&txids)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // Create a HashSet of all spent outpoints for fast lookup
        let mut spent_outpoints = HashSet::new();
        for tx in txs.iter() {
            for input in tx.input.iter() {
                spent_outpoints.insert(input.previous_output);
            }
        }

        // Find the first unspent output for this address
        for tx in txs.iter() {
            for (vout, output) in tx.output.iter().enumerate() {
                if output.script_pubkey == spk {
                    let outpoint = elements::OutPoint {
                        txid: tx.txid(),
                        vout: vout as u32,
                    };

                    // Check if this output is spent using the HashSet
                    if !spent_outpoints.contains(&outpoint) {
                        return Ok(Some((outpoint, output.clone())));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn get_genesis_hash(&self) -> Result<elements::BlockHash, Error> {
        let headers = self
            .inner
            .get_headers(&[0], &HashMap::new())
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        Ok(headers[0].block_hash())
    }

    async fn broadcast_tx(&self, signed_tx: &elements::Transaction) -> Result<String, Error> {
        let txid = self
            .inner
            .broadcast(signed_tx)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        Ok(txid.to_hex())
    }

    fn network(&self) -> LiquidChain {
        crate::elements_network_to_liquid_chain(self.network)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use boltz_client::{
        network::{LiquidChain, LiquidClient},
        ToHex,
    };
    use lwk_wollet::{
        asyncr::{self, EsploraClientBuilder},
        elements, ElementsNetwork,
    };

    use crate::clients::EsploraClient;

    #[tokio::test]
    #[ignore = "requires internet connection"]
    async fn test_esplora_client() {
        let client = asyncr::EsploraClient::new(
            ElementsNetwork::Liquid,
            "https://blockstream.info/liquid/api",
        );

        test_esplora_client_liquid(client).await;
    }

    #[tokio::test]
    #[ignore = "requires internet connection"]
    async fn test_waterfalls_client() {
        let client = EsploraClientBuilder::new(
            "https://waterfalls.liquidwebwallet.org/liquid/api",
            ElementsNetwork::Liquid,
        )
        .waterfalls(true)
        .build()
        .unwrap();
        test_esplora_client_liquid(client).await;
    }

    async fn test_esplora_client_liquid(raw_client: asyncr::EsploraClient) {
        let client = EsploraClient::from_client(Arc::new(raw_client), ElementsNetwork::Liquid);
        assert_eq!(client.network(), LiquidChain::Liquid);

        assert_eq!(
            client.get_genesis_hash().await.unwrap().to_hex(),
            "1466275836220db2944ca059a3a10ef6fd2ea684b0688d2c379296888a206003"
        );

        let address: elements::Address = "ex1qnv4dcfjn9jjww9q699vnjrwp879zkfl98zzyy5"
            .parse()
            .unwrap();
        // this test can start failing if the address utxo become spent, find another address to test with
        let r = client.get_address_utxo(&address).await.unwrap().unwrap();
        assert_eq!(
            r.0.txid.to_hex(),
            "22b1240eb51714a95e3819bb2d05b1c170aa72a974c529443bf697ae3700ff1f"
        );
        assert_eq!(r.0.vout, 0);
        assert_eq!(
            r.1.script_pubkey.to_hex(),
            "00149b2adc26532ca4e7141a2959390dc13f8a2b27e5"
        );
    }
}
