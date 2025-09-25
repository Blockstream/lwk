use async_trait::async_trait;
use boltz_client::elements;
use boltz_client::error::Error;
use boltz_client::network::LiquidChain;

pub struct ElectrumClient(lwk_wollet::ElectrumClient);

impl ElectrumClient {
    pub fn new(url: &str, tls: bool, validate_domain: bool) -> Result<Self, String> {
        let electrum_url = lwk_wollet::ElectrumUrl::new(url, tls, validate_domain)
            .map_err(|e| e.to_string())?;
        let client = lwk_wollet::ElectrumClient::new(&electrum_url)
            .map_err(|e| e.to_string())?;
        Ok(Self(client))
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
        todo!()
    }
}
