use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use crate::config::{
    LIQUID_DEFAULT_REGTEST_ASSET_STR, LIQUID_POLICY_ASSET_STR, LIQUID_TESTNET_POLICY_ASSET_STR,
};
use crate::domain::verify_domain_name;
use crate::elements::hashes::{sha256, Hash};
use crate::elements::{AssetId, ContractHash, OutPoint};
use crate::error::Error;
use crate::util::{serde_from_hex, serde_to_hex, verify_pubkey};
use crate::ElementsNetwork;
use elements::{Transaction, Txid};
use futures::{stream, StreamExt};
use once_cell::sync::Lazy;
use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

static RE_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[[:ascii:]]{1,255}$").expect("static"));
static RE_TICKER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9.\-]{3,24}$").expect("static"));

/// The entity of an asset, contains the domain of the issuer.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Entity {
    #[serde(rename = "domain")]
    Domain(String),
}

impl Entity {
    pub fn domain(&self) -> &str {
        match self {
            Entity::Domain(d) => d.as_str(),
        }
    }
}

// TODO: should we allow the caller to set extra arbitrary fields? if so how should we treat them?
// should we allow them to contribute to the contract hash, but we should skip validation for
// those? For instance how should we handle the nonce field that asset
// 123465c803ae336c62180e52d94ee80d80828db54df9bedbb9860060f49de2eb has?

// Order of the fields here determines the serialization order, make sure it's ordered
// lexicographically.

/// A contract defining metadata of an asset such the name and the ticker
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Contract {
    pub entity: Entity,
    #[serde(deserialize_with = "serde_from_hex", serialize_with = "serde_to_hex")]
    pub issuer_pubkey: Vec<u8>,
    pub name: String,
    pub precision: u8,
    pub ticker: String,
    pub version: u8,
}

impl Contract {
    pub fn from_value(value: &Value) -> Result<Self, Error> {
        Ok(serde_json::from_value(value.clone())?)
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.version != 0 {
            return Err(Error::InvalidVersion);
        }

        if self.precision > 8 {
            return Err(Error::InvalidPrecision);
        }

        if !RE_NAME.is_match(&self.name) {
            return Err(Error::InvalidName);
        }

        if !RE_TICKER.is_match(&self.ticker) {
            return Err(Error::InvalidTicker);
        }

        verify_pubkey(&self.issuer_pubkey).map_err(|_| Error::InvalidIssuerPubkey)?;

        let Entity::Domain(domain) = &self.entity;
        verify_domain_name(domain)?;

        Ok(())
    }

    pub fn contract_hash(&self) -> Result<ContractHash, Error> {
        let value = serde_json::to_value(self)?;
        contract_json_hash(&value)
    }
}

impl FromStr for Contract {
    type Err = Error;

    fn from_str(contract: &str) -> Result<Self, Self::Err> {
        let contract = serde_json::Value::from_str(contract)?;
        let contract = Contract::from_value(&contract)?;
        contract.validate()?;
        Ok(contract)
    }
}

#[derive(Clone)]
pub struct Registry {
    client: reqwest::Client,
    base_url: String,
}

pub struct RegistryCache {
    inner: Registry,
    cache: HashMap<AssetId, RegistryData>,
}

fn init_cache() -> HashMap<AssetId, RegistryData> {
    let mut cache = HashMap::new();
    cache.extend([lbtc(), tlbtc(), rlbtc()]);
    cache
}

impl RegistryCache {
    pub fn new_hardcoded(registry: Registry) -> Self {
        Self {
            inner: registry,
            cache: init_cache(),
        }
    }
    pub async fn new(registry: Registry, asset_ids: &[AssetId], concurrency: usize) -> Self {
        let mut cache = init_cache();
        let keys = cache.keys().cloned().collect::<Vec<_>>();

        let registry_clone = registry.clone();
        let mut stream = stream::iter(asset_ids.iter().filter(|e| !keys.contains(e)))
            .map(|&asset_id| {
                let registry = registry_clone.clone();
                async move { (asset_id, registry.fetch(asset_id).await.ok()) }
            })
            .buffer_unordered(concurrency);

        while let Some((asset_id, data)) = stream.next().await {
            if let Some(data) = data {
                cache.insert(asset_id, data);
            }
        }

        Self {
            inner: registry,
            cache,
        }
    }

    pub fn get(&self, asset_id: AssetId) -> Option<RegistryData> {
        self.cache.get(&asset_id).cloned()
    }

    pub async fn fetch_with_tx(
        &self,
        asset_id: AssetId,
        client: &crate::asyncr::EsploraClient,
    ) -> Result<(Contract, Transaction), Error> {
        self.inner.fetch_with_tx(asset_id, client).await
    }

    pub async fn post(&self, data: &RegistryPost) -> Result<(), Error> {
        self.inner.post(data).await
    }
}

#[derive(Serialize, Clone)]
pub struct RegistryPost {
    contract: Contract,
    asset_id: AssetId,
}

impl fmt::Display for RegistryPost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self).expect("failed to serialize registry post")
        )
    }
}

impl RegistryPost {
    pub fn new(contract: Contract, asset_id: AssetId) -> Self {
        Self { contract, asset_id }
    }
}

impl Registry {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn default_for_network(network: ElementsNetwork) -> Result<Self, Error> {
        Ok(Self::new(network_default_url(network)?))
    }

    pub async fn fetch(&self, asset_id: AssetId) -> Result<RegistryData, Error> {
        // TODO should discriminate between 404 and other errors
        let url = format!("{}/{}", self.base_url.trim_end_matches("/"), asset_id);
        let response = self.client.get(url).send().await?;
        let data = response.json::<RegistryData>().await?;
        Ok(data)
    }

    pub async fn fetch_with_tx(
        &self,
        asset_id: AssetId,
        client: &crate::asyncr::EsploraClient,
    ) -> Result<(Contract, Transaction), Error> {
        let data = self.fetch(asset_id).await?;
        let tx = client.get_transaction(data.issuance_txin.txid).await?;
        Ok((data.contract, tx))
    }

    pub async fn post(&self, data: &RegistryPost) -> Result<(), Error> {
        let response = self.client.post(&self.base_url).json(&data).send().await?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(Error::Generic(format!(
                "Failed to post contract to registry: {} {}",
                status, body
            )))
        }
    }
}

fn network_default_url(network: ElementsNetwork) -> Result<&'static str, Error> {
    Ok(match network {
        ElementsNetwork::Liquid => "https://assets.blockstream.info",
        ElementsNetwork::LiquidTestnet => "https://assets-testnet.blockstream.info",
        ElementsNetwork::ElementsRegtest { policy_asset: _ } => "http://127.0.0.1:3023",
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub mod blocking {
    use elements::{AssetId, Transaction};
    use tokio::runtime::Runtime;

    use crate::{ElementsNetwork, Error};

    use super::RegistryPost;

    pub struct Registry {
        inner: super::Registry,
        rt: Runtime,
    }

    impl Registry {
        pub fn new(base_url: &str) -> Result<Self, Error> {
            Ok(Self {
                inner: super::Registry::new(base_url),
                rt: Runtime::new()?,
            })
        }

        pub fn default_for_network(network: ElementsNetwork) -> Result<Self, Error> {
            Ok(Self {
                inner: super::Registry::new(super::network_default_url(network)?),
                rt: Runtime::new()?,
            })
        }

        pub fn fetch(&self, asset_id: AssetId) -> Result<super::RegistryData, Error> {
            self.rt.block_on(self.inner.fetch(asset_id))
        }

        pub fn fetch_with_tx(
            &self,
            asset_id: AssetId,
            client: &crate::asyncr::EsploraClient,
        ) -> Result<(super::Contract, Transaction), Error> {
            self.rt.block_on(self.inner.fetch_with_tx(asset_id, client))
        }

        pub fn post(&self, data: &RegistryPost) -> Result<(), Error> {
            self.rt.block_on(self.inner.post(data))
        }
    }
}

/// The asset id and reissuance token of the input
///
/// Fails if they do not commit to the contract.
pub fn asset_ids(txin: &elements::TxIn, contract: &Contract) -> Result<(AssetId, AssetId), Error> {
    let ch_from_txin = ContractHash::from_byte_array(txin.asset_issuance.asset_entropy);
    if contract.contract_hash()? != ch_from_txin {
        return Err(Error::ContractDoesNotCommitToAssetId);
    }
    Ok(txin.issuance_ids())
}

/// Compute the asset id and reissuance token id
///
/// The ids are derived from the contract.
/// This implicitly proves that the contract commits to the ids.
pub fn issuance_ids(
    contract: &Contract,
    issuance_prevout: OutPoint,
    is_confidential: bool,
) -> Result<(AssetId, AssetId), Error> {
    let entropy = AssetId::generate_asset_entropy(issuance_prevout, contract.contract_hash()?);
    let asset_id = AssetId::from_entropy(entropy);
    let token_id = AssetId::reissuance_token_from_entropy(entropy, is_confidential);
    Ok((asset_id, token_id))
}

pub fn contract_json_hash(contract: &Value) -> Result<ContractHash, Error> {
    let contract_str = serde_json::to_string(contract)?;

    // use the ContractHash representation for correct (reverse) hex encoding,
    // but use a single SHA256 instead of the double hash assumed by
    // ContractHash::hash()
    let hash = sha256::Hash::hash(contract_str.as_bytes());
    Ok(ContractHash::from_raw_hash(hash))
}

#[derive(Debug, Deserialize, Clone)]
pub struct TxIn {
    pub txid: Txid,
    pub vin: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegistryData {
    pub contract: Contract,
    pub issuance_txin: TxIn,
}

impl RegistryData {
    pub fn precision(&self) -> u8 {
        self.contract.precision
    }

    pub fn ticker(&self) -> &str {
        &self.contract.ticker
    }
}

/// Create a RegistryData mock for Liquid Bitcoin
fn lbtc() -> (AssetId, RegistryData) {
    let asset_id = AssetId::from_str(LIQUID_POLICY_ASSET_STR).expect("static");
    let data = RegistryData {
        contract: Contract {
            entity: Entity::Domain("".to_string()),
            issuer_pubkey: vec![2; 33],
            name: "Liquid Bitcoin".to_string(),
            precision: 8,
            ticker: "LBTC".to_string(),
            version: 0,
        },
        issuance_txin: TxIn {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("static"),
            vin: 0,
        },
    };
    (asset_id, data)
}

/// Create a RegistryData mock for TestnetLiquid Bitcoin
fn tlbtc() -> (AssetId, RegistryData) {
    let asset_id = AssetId::from_str(LIQUID_TESTNET_POLICY_ASSET_STR).expect("static");
    let data = RegistryData {
        contract: Contract {
            entity: Entity::Domain("".to_string()),
            issuer_pubkey: vec![2; 33],
            name: "Testnet Liquid Bitcoin".to_string(),
            precision: 8,
            ticker: "tLBTC".to_string(),
            version: 0,
        },
        issuance_txin: TxIn {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("static"),
            vin: 0,
        },
    };
    (asset_id, data)
}

/// Create a RegistryData mock for RegtestLiquid Bitcoin
fn rlbtc() -> (AssetId, RegistryData) {
    let asset_id = AssetId::from_str(LIQUID_DEFAULT_REGTEST_ASSET_STR).expect("static");
    let data = RegistryData {
        contract: Contract {
            entity: Entity::Domain("".to_string()),
            issuer_pubkey: vec![2; 33],
            name: "Regtest Liquid Bitcoin".to_string(),
            precision: 8,
            ticker: "rLBTC".to_string(),
            version: 0,
        },
        issuance_txin: TxIn {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("static"),
            vin: 0,
        },
    };
    (asset_id, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use elements::hex::FromHex;
    use std::str::FromStr;

    #[test]
    fn test_get_assets() {
        let registry_json_response = r#"{"asset_id":"8363084c77fbaebce672092d301fc103495546457468b88a0830ce4797562c03","contract":{"entity":{"domain":"nitramiz.github.io"},"issuer_pubkey":"02fd002ce3bb8bb5d626aec4b3821d100c0e2cae226f8199860767cb70b69a3305","name":"TestOps","precision":0,"ticker":"BSOPS","version":0},"issuance_txin":{"txid":"08186258abed0daa9a9d2a900c5e3d189235610887e3bda70f12cde11ba38747","vin":0},"issuance_prevout":{"txid":"ff0cbfa8d97a192a0e296451afee8028c9d414aae6dee145f4d71d35518c9962","vout":1},"version":0,"issuer_pubkey":"02fd002ce3bb8bb5d626aec4b3821d100c0e2cae226f8199860767cb70b69a3305","name":"TestOps","ticker":"BSOPS","precision":0,"entity":{"domain":"nitramiz.github.io"}}"#;
        let _: RegistryData = serde_json::from_str(registry_json_response).unwrap();
    }

    #[ignore = "require internet connection"]
    #[test]
    fn test_registry_fetch_blocking() {
        let tether_asset_id =
            AssetId::from_str("ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2")
                .unwrap();
        let registry = blocking::Registry::default_for_network(ElementsNetwork::Liquid).unwrap();
        let registry_data = registry.fetch(tether_asset_id).unwrap();
        assert_eq!(registry_data.contract.ticker, "USDt");
    }

    #[test]
    fn test_registry() {
        let contract_string = "{\"entity\":{\"domain\":\"tether.to\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Tether USD\",\"precision\":8,\"ticker\":\"USDt\",\"version\":0}";
        let contract_value = serde_json::Value::from_str(contract_string).unwrap();
        let contract = Contract::from_value(&contract_value).unwrap();
        contract.validate().unwrap();
        assert_eq!(
            serde_json::to_string(&contract).unwrap(),
            contract_string.to_string()
        );
        // From
        // https://blockstream.info/liquid/tx/abb4080d91849e933ee2ed65da6b436f7c385cf363fb4aa08399f1e27c58ff3d?input:0&expand
        assert_eq!(
            contract.contract_hash().unwrap().to_string(),
            "3c7f0a53c2ff5b99590620d7f6604a7a3a7bfbaaa6aa61f7bfc7833ca03cde82".to_string()
        );

        // Failing tests
        let mut contract = Contract::from_value(&contract_value).unwrap();

        contract.entity = Entity::Domain("Tether.to".to_string());
        assert!(contract.validate().is_err());
        contract.entity = Entity::Domain("tether.to".to_string());
        assert!(contract.validate().is_ok());

        contract.issuer_pubkey = vec![2];
        assert!(contract.validate().is_err());
        contract.issuer_pubkey = vec![2; 33];
        assert!(contract.validate().is_ok());

        contract.ticker = "US".to_string();
        assert!(contract.validate().is_err());
        contract.ticker = "USDt".to_string();
        assert!(contract.validate().is_ok());

        contract.name = "Tether USDü".to_string();
        assert!(contract.validate().is_err());
        contract.name = "Tether USD".to_string();
        assert!(contract.validate().is_ok());

        contract.precision = 9;
        assert!(contract.validate().is_err());
        contract.precision = 8;
        assert!(contract.validate().is_ok());

        contract.version = 1;
        assert!(contract.validate().is_err());
        contract.version = 0;
        assert!(contract.validate().is_ok());

        // https://blockstream.info/liquid/api/tx/abb4080d91849e933ee2ed65da6b436f7c385cf363fb4aa08399f1e27c58ff3d/hex
        let tx_hex = include_str!("../tests/data/usdt-issuance-tx.hex");
        let tx: elements::Transaction =
            elements::encode::deserialize(&Vec::<u8>::from_hex(tx_hex).unwrap()).unwrap();

        let asset_usdt = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
        let token_usdt = "59fe4d2127ba9f16bd6850a3e6271a166e7ed2e1669f6c107d655791c94ee98f";

        let mut contract = Contract::from_value(&contract_value).unwrap();
        let (asset, token) = asset_ids(&tx.input[0], &contract).unwrap();
        assert_eq!(&asset.to_string(), asset_usdt);
        assert_eq!(&token.to_string(), token_usdt);

        let issuance_prevout = tx.input[0].previous_output;
        let is_confidential = tx.input[0].asset_issuance.amount.is_confidential();
        let (asset, token) = issuance_ids(&contract, issuance_prevout, is_confidential).unwrap();
        assert_eq!(&asset.to_string(), asset_usdt);
        assert_eq!(&token.to_string(), token_usdt);

        // Error cases
        contract.version = 1;
        assert!(asset_ids(&tx.input[0], &contract).is_err());
    }

    #[tokio::test]
    async fn test_registry_cache_hardcoded() {
        let registry = Registry::default_for_network(ElementsNetwork::default_regtest()).unwrap();
        let cache = RegistryCache::new(registry, &[], 1).await;
        // policy assets of regtest(default)/testnet/mainnet network are hard coded
        let regtest_asset_id = AssetId::from_str(LIQUID_DEFAULT_REGTEST_ASSET_STR).unwrap();
        let testnet_asset_id = AssetId::from_str(LIQUID_TESTNET_POLICY_ASSET_STR).unwrap();
        let mainnet_asset_id = AssetId::from_str(LIQUID_POLICY_ASSET_STR).unwrap();
        assert!(cache.get(regtest_asset_id).is_some());
        assert!(cache.get(testnet_asset_id).is_some());
        assert!(cache.get(mainnet_asset_id).is_some());
    }

    #[ignore = "require internet connection"]
    #[tokio::test]
    async fn test_registry_cache() {
        let registry = Registry::default_for_network(ElementsNetwork::Liquid).unwrap();
        let asset_id =
            AssetId::from_str("ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2")
                .unwrap();
        let cache = RegistryCache::new(registry, &[asset_id], 1).await;
        let data = cache.get(asset_id).unwrap();
        assert_eq!(data.contract.ticker, "USDt");
        assert_eq!(data.contract.precision, 8);

        let registry = Registry::default_for_network(ElementsNetwork::Liquid).unwrap();
        let cache_2 = RegistryCache::new(registry, &[], 1).await;
        assert!(cache_2.get(asset_id).is_none());
    }
}
