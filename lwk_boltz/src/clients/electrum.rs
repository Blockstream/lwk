use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use boltz_client::elements;
use boltz_client::error::Error;
use boltz_client::network::LiquidChain;
use boltz_client::ToHex;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::ElementsNetwork;
use tokio::task;

#[derive(Clone)]
pub struct ElectrumClient {
    inner: Arc<lwk_wollet::ElectrumClient>,
    network: ElementsNetwork,
}

impl ElectrumClient {
    pub fn from_client(client: lwk_wollet::ElectrumClient, network: ElementsNetwork) -> Self {
        Self {
            inner: Arc::new(client),
            network,
        }
    }

    pub fn new(
        url: &str,
        tls: bool,
        validate_domain: bool,
        network: ElementsNetwork,
    ) -> Result<Self, String> {
        let electrum_url =
            lwk_wollet::ElectrumUrl::new(url, tls, validate_domain).map_err(|e| e.to_string())?;
        let client = lwk_wollet::ElectrumClient::new(&electrum_url).map_err(|e| e.to_string())?;
        Ok(Self {
            inner: Arc::new(client),
            network,
        })
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl boltz_client::network::LiquidClient for ElectrumClient {
    async fn get_address_utxo(
        &self,
        address: &elements::Address,
    ) -> Result<Option<(elements::OutPoint, elements::TxOut)>, Error> {
        let spk = address.script_pubkey();
        let inner = Arc::clone(&self.inner);
        let spk_clone = spk.clone();
        let history = task::spawn_blocking(move || inner.get_scripts_history(&[&spk_clone]))
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?
            .map_err(|e| Error::Protocol(e.to_string()))?;

        if history.is_empty() || history[0].is_empty() {
            return Ok(None);
        }

        let txids: Vec<_> = history[0].iter().map(|h| h.txid).collect();
        let inner = Arc::clone(&self.inner);
        let txs = task::spawn_blocking(move || inner.get_transactions(&txids))
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?
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
        let inner = Arc::clone(&self.inner);
        let headers = task::spawn_blocking(move || {
            inner
                .get_headers(&[0], &HashMap::new())
                .map_err(|e| Error::Protocol(e.to_string()))
        })
        .await
        .map_err(|e| Error::Protocol(e.to_string()))??;
        Ok(headers[0].block_hash())
    }

    async fn broadcast_tx(&self, signed_tx: &elements::Transaction) -> Result<String, Error> {
        let inner = Arc::clone(&self.inner);
        let tx = signed_tx.clone();
        let txid = task::spawn_blocking(move || {
            inner
                .broadcast(&tx)
                .map_err(|e| Error::Protocol(e.to_string()))
        })
        .await
        .map_err(|e| Error::Protocol(e.to_string()))??;
        Ok(txid.to_hex())
    }

    fn network(&self) -> LiquidChain {
        crate::elements_network_to_liquid_chain(self.network)
    }
}

#[cfg(test)]
mod tests {
    use boltz_client::{
        network::{LiquidChain, LiquidClient},
        ToHex,
    };
    use lwk_wollet::{elements, ElementsNetwork};

    use crate::clients::ElectrumClient;

    #[tokio::test]
    #[ignore = "requires internet connection"]
    async fn test_electrum_client() {
        let client = ElectrumClient::new(
            "elements-mainnet.blockstream.info:50002",
            true,
            true,
            ElementsNetwork::Liquid,
        )
        .unwrap();
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

    #[tokio::test]
    #[ignore = "requires regtest env"]
    async fn test_electrum_client_regtest() {
        let client =
            ElectrumClient::new("localhost:19002", false, false, ElementsNetwork::Liquid).unwrap();
        assert_eq!(
            client.get_genesis_hash().await.unwrap().to_hex(),
            "00902a6b70c2ca83b5d9c815d96a0e2f4202179316970d14ea1847dae5b1ca21"
        );
    }
}
