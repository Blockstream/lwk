use async_trait::async_trait;
use boltz_client::elements;
use boltz_client::error::Error;
use boltz_client::network::LiquidChain;

struct ElectrumClient(lwk_wollet::ElectrumClient);

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
