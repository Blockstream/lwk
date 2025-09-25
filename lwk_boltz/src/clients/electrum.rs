use std::collections::HashMap;

use async_trait::async_trait;
use boltz_client::elements;
use boltz_client::error::Error;
use boltz_client::network::LiquidChain;
use lwk_wollet::blocking::BlockchainBackend;
use lwk_wollet::ElementsNetwork;

pub struct ElectrumClient {
    inner: lwk_wollet::ElectrumClient,
    network: ElementsNetwork,
}

impl ElectrumClient {
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
            inner: client,
            network,
        })
    }
}

#[async_trait]
impl boltz_client::network::LiquidClient for ElectrumClient {
    async fn get_address_utxo(
        &self,
        _address: &elements::Address,
    ) -> Result<Option<(elements::OutPoint, elements::TxOut)>, Error> {
        todo!()
    }

    async fn get_genesis_hash(&self) -> Result<elements::BlockHash, Error> {
        // TODO this should use spawn_blocking
        let headers = self
            .inner
            .get_headers(&[0], &HashMap::new())
            .map_err(|e| Error::Protocol(e.to_string()))?;
        Ok(headers[0].block_hash())
    }

    async fn broadcast_tx(&self, _signed_tx: &elements::Transaction) -> Result<String, Error> {
        todo!()
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
    use lwk_wollet::ElementsNetwork;

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
    }
}
