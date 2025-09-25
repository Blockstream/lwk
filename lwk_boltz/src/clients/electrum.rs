use async_trait::async_trait;
use boltz_client::elements;
use boltz_client::error::Error;
use boltz_client::network::LiquidChain;
use lwk_wollet::ElementsNetwork;

pub struct ElectrumClient {
    inner: lwk_wollet::ElectrumClient,
    network: ElementsNetwork,
}

impl ElectrumClient {
    pub fn new(url: &str, tls: bool, validate_domain: bool, network: ElementsNetwork) -> Result<Self, String> {
        let electrum_url = lwk_wollet::ElectrumUrl::new(url, tls, validate_domain)
            .map_err(|e| e.to_string())?;
        let client = lwk_wollet::ElectrumClient::new(&electrum_url)
            .map_err(|e| e.to_string())?;
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
        todo!()
    }

    async fn broadcast_tx(&self, _signed_tx: &elements::Transaction) -> Result<String, Error> {
        todo!()
    }

    fn network(&self) -> LiquidChain {
        crate::elements_network_to_liquid_chain(self.network)
    }
}
