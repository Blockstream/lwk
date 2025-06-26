use lwk_wollet::{elements, registry::RegistryCache};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{
    blockdata::asset_id::AssetIds, AssetId, Contract, Error, EsploraClient, Network, Transaction,
};

#[wasm_bindgen]
pub struct Registry {
    inner: lwk_wollet::registry::RegistryCache,
}

#[wasm_bindgen]
pub struct RegistryData {
    inner: lwk_wollet::registry::RegistryData,
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

impl From<lwk_wollet::registry::RegistryCache> for Registry {
    fn from(inner: lwk_wollet::registry::RegistryCache) -> Self {
        Self { inner }
    }
}

impl From<Registry> for lwk_wollet::registry::RegistryCache {
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

impl From<lwk_wollet::registry::RegistryData> for RegistryData {
    fn from(inner: lwk_wollet::registry::RegistryData) -> Self {
        Self { inner }
    }
}

impl From<RegistryData> for lwk_wollet::registry::RegistryData {
    fn from(inner: RegistryData) -> Self {
        inner.inner
    }
}

#[wasm_bindgen]
impl Registry {
    pub async fn new(url: &str, asset_ids: &AssetIds) -> Result<Self, Error> {
        let registry = lwk_wollet::registry::Registry::new(url);
        let asset_ids: Vec<elements::AssetId> = asset_ids.into();
        Ok(RegistryCache::new(registry, &asset_ids, 4).await.into())
    }

    #[wasm_bindgen(js_name = defaultForNetwork)]
    pub async fn default_for_network(
        network: &Network,
        asset_ids: &AssetIds,
    ) -> Result<Self, Error> {
        let registry = lwk_wollet::registry::Registry::default_for_network(network.into())?;
        let asset_ids: Vec<elements::AssetId> = asset_ids.into();
        let cache = RegistryCache::new(registry, &asset_ids, 1).await;
        Ok(cache.into())
    }

    #[wasm_bindgen(js_name = defaultHardcodedForNetwork)]
    pub fn default_hardcoded_for_network(network: &Network) -> Result<Self, Error> {
        let registry = lwk_wollet::registry::Registry::default_for_network(network.into())?;
        let cache = RegistryCache::new_hardcoded(registry);
        Ok(cache.into())
    }

    #[wasm_bindgen(js_name = fetchWithTx)]
    pub async fn fetch_with_tx(
        &self,
        asset_id: &AssetId,
        client: &EsploraClient,
    ) -> Result<AssetMeta, Error> {
        let (contract, tx) = self
            .inner
            .fetch_with_tx((*asset_id).into(), client.as_ref())
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

    pub fn get(&self, asset_id: &AssetId) -> Option<RegistryData> {
        self.inner.get((*asset_id).into()).map(|data| data.into())
    }
}

#[wasm_bindgen]
impl RegistryData {
    pub fn precision(&self) -> u8 {
        self.inner.precision()
    }

    pub fn ticker(&self) -> String {
        self.inner.ticker().to_string()
    }
}
