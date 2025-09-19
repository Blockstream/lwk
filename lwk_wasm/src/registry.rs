use lwk_wollet::{elements, registry::RegistryCache};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{
    blockdata::asset_id::AssetIds, AssetId, Contract, Error, EsploraClient, Network, Pset,
    Transaction,
};

/// A Registry, a repository to store and retrieve asset metadata, like the name or the ticker of an asset.
#[wasm_bindgen]
pub struct Registry {
    inner: lwk_wollet::registry::RegistryCache,
}

#[wasm_bindgen]
pub struct RegistryData {
    inner: lwk_wollet::registry::RegistryData,
}

/// Data related to an asset in the registry:
/// - contract: the contract of the asset
/// - tx: the issuance transaction of the asset
#[wasm_bindgen]
pub struct AssetMeta {
    contract: Contract,
    tx: Transaction,
}

/// The data to post to the registry to publish a contract for an asset id
#[wasm_bindgen]
#[derive(Clone)]
pub struct RegistryPost {
    inner: lwk_wollet::registry::RegistryPost,
}

#[wasm_bindgen]
impl AssetMeta {
    /// Return the contract of the asset.
    pub fn contract(&self) -> Contract {
        self.contract.clone()
    }

    /// Return the issuance transaction of the asset.
    pub fn tx(&self) -> Transaction {
        self.tx.clone()
    }
}

#[wasm_bindgen]
impl RegistryPost {
    /// Create a new registry post object to be used to publish a contract for an asset id in the registry.
    #[wasm_bindgen(constructor)]
    pub fn new(contract: Contract, asset_id: AssetId) -> Self {
        lwk_wollet::registry::RegistryPost::new(contract.into(), asset_id.into()).into()
    }

    /// Return a string representation of the registry post (mostly for debugging).
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
    /// Create a new registry cache specifying the URL of the registry,
    /// fetch the assets metadata identified by the given asset ids and cache them for later local retrieval.
    /// Use `default_for_network()` to get the default registry for the given network.
    pub async fn new(url: &str, asset_ids: &AssetIds) -> Result<Self, Error> {
        let registry = lwk_wollet::registry::Registry::new(url);
        let asset_ids: Vec<elements::AssetId> = asset_ids.into();
        Ok(RegistryCache::new(registry, &asset_ids, 4).await.into())
    }

    /// Return the default registry for the given network,
    /// fetch the assets metadata identified by the given asset ids and cache them for later local retrieval.
    /// Use `new()` to specify a custom URL
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

    /// Create a new registry cache, using only the hardcoded assets.
    ///
    /// Hardcoded assets are the policy assets (LBTC, tLBTC, rLBTC) and the USDT asset on mainnet.
    #[wasm_bindgen(js_name = defaultHardcodedForNetwork)]
    pub fn default_hardcoded_for_network(network: &Network) -> Result<Self, Error> {
        let registry = lwk_wollet::registry::Registry::default_for_network(network.into())?;
        let cache = RegistryCache::new_hardcoded(registry);
        Ok(cache.into())
    }

    /// Fetch the contract and the issuance transaction of the given asset id from the registry
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

    /// Post a contract to the registry for registration.
    pub async fn post(&self, data: &RegistryPost) -> Result<(), Error> {
        let data: lwk_wollet::registry::RegistryPost = data.clone().into();
        Ok(self.inner.post(&data).await?)
    }

    /// Return the asset metadata related to the given asset id if it exists in this registry.
    pub fn get(&self, asset_id: &AssetId) -> Option<RegistryData> {
        self.inner.get((*asset_id).into()).map(|data| data.into())
    }

    /// Return the asset metadata related to the given token id,
    /// in other words `token_id` is the reissuance token of the returned asset
    #[wasm_bindgen(js_name = getAssetOfToken)]
    pub fn get_asset_of_token(&self, token_id: &AssetId) -> Option<RegistryData> {
        self.inner
            .get_asset_of_token((*token_id).into())
            .map(Into::into)
    }

    /// Add the contracts information of the assets used in the Pset
    /// if available in this registry.
    /// Without the contract information, the partially signed transaction
    /// is valid but will not show asset information when signed with an hardware wallet.
    #[wasm_bindgen(js_name = addContracts)]
    pub fn add_contracts(&self, pset: Pset) -> Result<Pset, Error> {
        let mut pset: elements::pset::PartiallySignedTransaction = pset.into();
        lwk_wollet::registry::add_contracts(&mut pset, self.inner.registry_asset_data().iter());
        Ok(pset.into())
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

    pub fn name(&self) -> String {
        self.inner.name().to_string()
    }

    pub fn domain(&self) -> String {
        self.inner.domain().to_string()
    }
}
