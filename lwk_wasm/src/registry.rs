use wasm_bindgen::prelude::wasm_bindgen;

use crate::{AssetId, Contract, Error, EsploraClient, Network, Transaction};

#[wasm_bindgen]
pub struct Registry {
    inner: lwk_wollet::registry::Registry,
}

#[wasm_bindgen]
pub struct AssetMeta {
    contract: Contract,
    tx: Transaction,
}

#[wasm_bindgen]
impl AssetMeta {
    pub fn contract(&self) -> Contract {
        self.contract.clone()
    }

    pub fn tx(&self) -> Transaction {
        self.tx.clone()
    }
}

impl From<lwk_wollet::registry::Registry> for Registry {
    fn from(inner: lwk_wollet::registry::Registry) -> Self {
        Self { inner }
    }
}

impl From<Registry> for lwk_wollet::registry::Registry {
    fn from(inner: Registry) -> Self {
        inner.inner
    }
}

#[wasm_bindgen]
impl Registry {
    pub fn new(url: &str) -> Result<Self, Error> {
        Ok(lwk_wollet::registry::Registry::new(url).into())
    }

    #[wasm_bindgen(js_name = defaultForNetwork)]
    pub fn default_for_network(network: &Network) -> Result<Self, Error> {
        let inner = lwk_wollet::registry::Registry::default_for_network(network.into())?;
        Ok(inner.into())
    }

    #[wasm_bindgen(js_name = fetchWithTx)]
    pub async fn fetch_with_tx(
        &self,
        asset_id: AssetId,
        client: &EsploraClient,
    ) -> Result<AssetMeta, Error> {
        let (contract, tx) = self
            .inner
            .fetch_with_tx(asset_id.into(), client.as_ref())
            .await?;
        Ok(AssetMeta {
            contract: contract.into(),
            tx: tx.into(),
        })
    }

    pub async fn post(&self, contract: &Contract, asset_id: AssetId) -> Result<(), Error> {
        Ok(self
            .inner
            .post(&contract.clone().into(), asset_id.into())
            .await?)
    }
}
