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
#[derive(Clone)]
pub struct RegistryPost {
    inner: lwk_wollet::registry::RegistryPost,
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

#[wasm_bindgen]
impl RegistryPost {
    #[wasm_bindgen(constructor)]
    pub fn new(contract: Contract, asset_id: AssetId) -> Self {
        lwk_wollet::registry::RegistryPost::new(contract.into(), asset_id.into()).into()
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self.inner)
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

impl From<lwk_wollet::registry::RegistryPost> for RegistryPost {
    fn from(inner: lwk_wollet::registry::RegistryPost) -> Self {
        Self { inner }
    }
}

impl From<RegistryPost> for lwk_wollet::registry::RegistryPost {
    fn from(inner: RegistryPost) -> Self {
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
        asset_id: &AssetId,
        client: &EsploraClient,
    ) -> Result<AssetMeta, Error> {
        let (contract, tx) = self
            .inner
            .fetch_with_tx(asset_id.clone().into(), client.as_ref())
            .await?;
        Ok(AssetMeta {
            contract: contract.into(),
            tx: tx.into(),
        })
    }

    pub async fn post(&self, data: &RegistryPost) -> Result<(), Error> {
        let data: lwk_wollet::registry::RegistryPost = data.clone().into();
        Ok(self.inner.post(&data).await?)
    }
}
