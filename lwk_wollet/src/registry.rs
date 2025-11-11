//! Registry related functions
//!
//! The Registry is repository to store and retrieve asset metadata, like the name or the ticker of an asset.

use std::collections::{HashMap, HashSet};
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
use elements::hashes::sha256::Midstate;
use elements::pset::elip100::AssetMetadata;
use elements::pset::elip100::TokenMetadata;
use elements::pset::PartiallySignedTransaction;
use elements::{AssetIssuance, LockTime, Script, Sequence, Transaction, TxInWitness, Txid};
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
    /// Contain the domain of the entity, such as `example.com`
    #[serde(rename = "domain")]
    Domain(String),
}

impl Entity {
    /// Get the domain of the entity, such as `example.com`
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
    /// The entity of the asset, such as the domain of the issuer
    pub entity: Entity,

    #[serde(deserialize_with = "serde_from_hex", serialize_with = "serde_to_hex")]
    /// The public key of the issuer, 33 bytes long.
    pub issuer_pubkey: Vec<u8>,

    /// The name of the asset
    ///
    /// The name must be 1 to 255 characters long and can only contain ASCII characters.
    pub name: String,

    /// The precision of the asset, such as 8 for Liquid Bitcoin.
    /// 100 satoshi of an assets with precision 2 is shown as "1.00"
    /// Maximum precision is 8.
    pub precision: u8,

    /// The ticker of the asset.
    ///
    /// The ticker must be 3 to 24 characters long and can only contain letters, numbers, dots and hyphens.
    pub ticker: String,

    /// The version of the contract, currently only 0 is supported
    pub version: u8,
}

impl Contract {
    /// Create a new contract from a JSON value, doesn't validate the contract, use [`Self::validate()`] to validate the contract.
    pub fn from_value(value: &Value) -> Result<Self, Error> {
        Ok(serde_json::from_value(value.clone())?)
    }

    /// Validate the contract against the rules of the registry
    ///
    /// If this method doesn't error the contract is semantically valid.
    /// Its publication can still fail when published if the proof on the domain is not valid.
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

    /// Compute the hash of the contract from its JSON representation
    ///
    /// The asset id and the reissuance token id are committed to this hash.
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

/// An asyncronous registry client, allowing to fetch and post assets metadata from the registry.
#[derive(Clone)]
pub struct Registry {
    client: reqwest::Client,
    base_url: String,
}

/// A registry cache contains a reference to the registry, and some cached asset metadata, hardcoded and fetched from the registry.
/// It also contains a token cache, to quickly find the asset id of a reissuance token.
pub struct RegistryCache {
    inner: Registry,

    /// contains the cached registry data
    cache: HashMap<AssetId, RegistryData>,

    /// for every asset, we compute the token_id and cache it here
    token_cache: HashMap<AssetId, AssetId>, // token_id -> asset_id
}

fn init_cache() -> (HashMap<AssetId, RegistryData>, HashMap<AssetId, AssetId>) {
    let mut cache = HashMap::new();
    let mut token_cache = HashMap::new();
    let usdt = usdt();
    let usdt_token = usdt.1.token_id().expect("static");
    token_cache.insert(usdt_token, usdt.0);
    cache.extend([lbtc(), tlbtc(), rlbtc(), usdt]);

    (cache, token_cache)
}

impl RegistryCache {
    /// Create a new registry cache, using only the hardcoded assets.
    ///
    /// Hardcoded assets are the policy assets (LBTC, tLBTC, rLBTC) and the USDT asset on mainnet.
    pub fn new_hardcoded(registry: Registry) -> Self {
        let (cache, token_cache) = init_cache();
        Self {
            inner: registry,
            cache,
            token_cache,
        }
    }

    /// Create a new registry cache, fetching the given `asset_ids` metadata from the given registry with the given `concurrency`.
    pub async fn new(registry: Registry, asset_ids: &[AssetId], concurrency: usize) -> Self {
        let (mut cache, mut token_cache) = init_cache();
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
                if let Ok(token_id) = data.token_id() {
                    token_cache.insert(token_id, asset_id);
                }
                cache.insert(asset_id, data);
            }
        }

        Self {
            inner: registry,
            cache,
            token_cache,
        }
    }

    /// Return the asset metadata related to the given asset id if it exists in the cache
    pub fn get(&self, asset_id: AssetId) -> Option<RegistryData> {
        self.cache.get(&asset_id).cloned()
    }

    /// Return the asset metadata related to the given token id,
    /// in other words `token_id` is the reissuance token of the returned asset
    pub fn get_asset_of_token(&self, token_id: AssetId) -> Option<RegistryData> {
        self.token_cache
            .get(&token_id)
            .and_then(|asset_id| self.cache.get(asset_id).cloned())
    }

    /// Fetch the contract and the issuance transaction of the given asset id from the registry
    pub async fn fetch_with_tx(
        &self,
        asset_id: AssetId,
        client: &crate::asyncr::EsploraClient,
    ) -> Result<(Contract, Transaction), Error> {
        self.inner.fetch_with_tx(asset_id, client).await
    }

    /// Post a contract to the registry
    pub async fn post(&self, data: &RegistryPost) -> Result<(), Error> {
        self.inner.post(data).await
    }

    /// Returns a list of registry asset data but with a dummy tx for the issuance tx
    /// because it's used for adding contracts to the pset and the full transaction is not needed there.
    /// TODO: fix this ugly hack
    pub fn registry_asset_data(&self) -> Vec<RegistryAssetData> {
        let mut result = vec![];
        for registry_data in self.cache.values() {
            if let (Ok(asset_id), Ok(token_id)) =
                (registry_data.asset_id(), registry_data.token_id())
            {
                let mut dummy_inputs: Vec<elements::TxIn> = vec![];
                let dummy_input = elements::TxIn {
                    previous_output: OutPoint::new(
                        registry_data.issuance_prevout.txid,
                        registry_data.issuance_prevout.vout,
                    ),
                    is_pegin: false,
                    script_sig: Script::new(),
                    sequence: Sequence::MAX,
                    asset_issuance: AssetIssuance::default(),
                    witness: TxInWitness::default(),
                };

                for _ in 0..registry_data.issuance_txin.vin + 1 {
                    dummy_inputs.push(dummy_input.clone());
                }
                let dummy_tx = Transaction {
                    version: 0,
                    lock_time: LockTime::ZERO,
                    input: dummy_inputs,
                    output: vec![],
                };
                let registry_asset_data = RegistryAssetData {
                    asset_id,
                    token_id,
                    issuance_vin: registry_data.issuance_txin.vin,
                    issuance_tx: dummy_tx.clone(),
                    contract: registry_data.contract.clone(),
                };
                result.push(registry_asset_data);
            }
        }

        result
    }
}

/// The data to post to the registry to publish a contract for an asset id
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
    /// Create a new registry post to publish a contract for an asset id
    pub fn new(contract: Contract, asset_id: AssetId) -> Self {
        Self { contract, asset_id }
    }
}

impl Registry {
    /// Create a new registry with the given base URL, use [`Self::default_for_network()`] to get the default registry for the given network
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
        }
    }

    /// Return the default registry for the given network, use [`Self::new()`] to specify a custom URL
    pub fn default_for_network(network: ElementsNetwork) -> Result<Self, Error> {
        Ok(Self::new(network_default_url(network)?))
    }

    /// Fetch the contract, the issuance transaction and the issuance prevout of the given asset id from the registry
    pub async fn fetch(&self, asset_id: AssetId) -> Result<RegistryData, Error> {
        // TODO should discriminate between 404 and other errors
        let url = format!("{}/{}", self.base_url.trim_end_matches("/"), asset_id);
        let response = self.client.get(url).send().await?;
        let data = response.json::<RegistryData>().await?;
        Ok(data)
    }

    /// Fetch the contract and the issuance transaction of the given asset id from the registry
    pub async fn fetch_with_tx(
        &self,
        asset_id: AssetId,
        client: &crate::asyncr::EsploraClient,
    ) -> Result<(Contract, Transaction), Error> {
        let data = self.fetch(asset_id).await?;
        let tx = client.get_transaction(data.issuance_txin.txid).await?;
        Ok((data.contract, tx))
    }

    /// Post a contract to the registry
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
    //! The module contains a blocking registry client, allowing to fetch and post assets metadata from the registry.
    //! The blocking client is based on the async client, it uses a tokio runtime to run the async client in a blocking context.

    use elements::{AssetId, Transaction};
    use tokio::runtime::Runtime;

    use crate::{ElementsNetwork, Error};

    use super::RegistryPost;

    /// A blocking registry client, allowing to fetch and post assets metadata from the registry.
    pub struct Registry {
        inner: super::Registry,
        rt: Runtime,
    }

    impl Registry {
        /// Create a new registry with the given base URL, use [`Self::default_for_network()`] to get the default registry for the given network
        pub fn new(base_url: &str) -> Result<Self, Error> {
            Ok(Self {
                inner: super::Registry::new(base_url),
                rt: Runtime::new()?,
            })
        }

        /// Return the default registry for the given network, use [`Self::new()`] to specify a custom URL
        pub fn default_for_network(network: ElementsNetwork) -> Result<Self, Error> {
            Ok(Self {
                inner: super::Registry::new(super::network_default_url(network)?),
                rt: Runtime::new()?,
            })
        }

        /// Fetch the contract, the issuance transaction and the issuance prevout of the given asset id from the registry
        pub fn fetch(&self, asset_id: AssetId) -> Result<super::RegistryData, Error> {
            self.rt.block_on(self.inner.fetch(asset_id))
        }

        /// Fetch the contract and the issuance transaction of the given asset id from the registry
        pub fn fetch_with_tx(
            &self,
            asset_id: AssetId,
            client: &impl crate::clients::blocking::BlockchainBackend,
        ) -> Result<(super::Contract, Transaction), Error> {
            let data = self.fetch(asset_id)?;
            let tx = client.get_transaction(data.issuance_txin.txid)?;
            Ok((data.contract, tx))
        }

        /// Post a contract to the registry
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

/// Compute the hash of the contract from its JSON representation
///
/// The asset id and the reissuance token id are committed to this hash.
pub fn contract_json_hash(contract: &Value) -> Result<ContractHash, Error> {
    let contract_str = serde_json::to_string(contract)?;

    // use the ContractHash representation for correct (reverse) hex encoding,
    // but use a single SHA256 instead of the double hash assumed by
    // ContractHash::hash()
    let hash = sha256::Hash::hash(contract_str.as_bytes());
    Ok(ContractHash::from_raw_hash(hash))
}

/// The input containing the issuance
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct TxIn {
    /// The transaction id of the transaction containing the issuance
    pub txid: Txid,
    /// The input index of the input containing the issuance
    pub vin: u32,
}

/// The data related to an issued asset with a contract in the registry.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct RegistryData {
    /// The contract of the asset, with details about the asset such as the name, the ticker, the precision, etc.
    pub contract: Contract,

    /// The input containing the issuance
    pub issuance_txin: TxIn,

    /// The outpoint creating the issuance (the output spent to create the issuance)
    pub issuance_prevout: IssuancePrevout,
}

/// The outpoint creating the issuance (the output spent to create the issuance)
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct IssuancePrevout {
    /// Get the transaction id of the outpoint creating the issuance
    pub txid: Txid,
    /// Get the output index of the outpoint creating the issuance
    pub vout: u32,
}

impl RegistryData {
    /// Get the precision of the asset as specified in the contract.
    ///
    /// The precision is the number of decimals of the asset. For example, 8 for Liquid Bitcoin.
    ///
    /// 100 satoshi of an assets with precision 2 is shown as "1.00"
    pub fn precision(&self) -> u8 {
        self.contract.precision
    }

    /// Get the ticker of the asset as specified in the contract.
    pub fn ticker(&self) -> &str {
        &self.contract.ticker
    }

    /// Get the name of the asset as specified in the contract.
    pub fn name(&self) -> &str {
        &self.contract.name
    }

    /// Get the domain of the asset as specified in the contract.
    /// The registry doesn't allow to publish an asset with a domain without a proof on the domain itself.
    pub fn domain(&self) -> &str {
        match &self.contract.entity {
            Entity::Domain(domain) => domain,
        }
    }

    /// Get the issuance transaction previous output.
    pub fn issuance_prevout(&self) -> OutPoint {
        OutPoint {
            txid: self.issuance_prevout.txid,
            vout: self.issuance_prevout.vout,
        }
    }

    /// Get the entropy of the issuance transaction, used to compute the asset id and the reissuance token id.
    pub fn entropy(&self) -> Result<Midstate, Error> {
        Ok(AssetId::generate_asset_entropy(
            self.issuance_prevout(),
            self.contract.contract_hash()?,
        ))
    }

    /// Get the asset id of this asset
    pub fn asset_id(&self) -> Result<AssetId, Error> {
        let entropy = self.entropy()?;
        Ok(AssetId::from_entropy(entropy))
    }

    /// Get the asset id of the reissuance token of this asset
    pub fn token_id(&self) -> Result<AssetId, Error> {
        let entropy = self.entropy()?;
        Ok(AssetId::reissuance_token_from_entropy(
            entropy, false, // TODO
        ))
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
        issuance_prevout: IssuancePrevout {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("static"),
            vout: 0,
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
        issuance_prevout: IssuancePrevout {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("static"),
            vout: 0,
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
        issuance_prevout: IssuancePrevout {
            txid: Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("static"),
            vout: 0,
        },
    };
    (asset_id, data)
}

fn usdt() -> (AssetId, RegistryData) {
    let asset_id =
        AssetId::from_str("ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2")
            .expect("static");
    let data = RegistryData {
        contract: Contract {
            entity: Entity::Domain("tether.to".to_string()),
            issuer_pubkey: vec![
                3, 55, 204, 238, 192, 190, 234, 2, 50, 235, 225, 76, 186, 1, 151, 169, 251, 212,
                95, 207, 46, 201, 70, 116, 157, 233, 32, 231, 20, 52, 194, 185, 4,
            ],
            name: "Tether USD".to_string(),
            precision: 8,
            ticker: "USDt".to_string(),
            version: 0,
        },
        issuance_txin: TxIn {
            txid: Txid::from_str(
                "abb4080d91849e933ee2ed65da6b436f7c385cf363fb4aa08399f1e27c58ff3d",
            )
            .expect("static"),
            vin: 0,
        },
        issuance_prevout: IssuancePrevout {
            txid: Txid::from_str(
                "9596d259270ef5bac0020435e6d859aea633409483ba64e232b8ba04ce288668",
            )
            .expect("static"),
            vout: 0,
        },
    };
    (asset_id, data)
}

/// Add the contracts information of the assets used in the Pset
/// if available in the given `assets` parameter.
/// Without the contract information, the partially signed transaction
/// is valid but will not show asset information when signed with an hardware wallet.
pub fn add_contracts<'a>(
    pset: &mut PartiallySignedTransaction,
    assets: impl Iterator<Item = &'a RegistryAssetData>,
) {
    let assets_in_pset: HashSet<_> = pset.outputs().iter().filter_map(|o| o.asset).collect();
    for registry_data in assets {
        // Policy asset and reissuance tokens do not require the contract
        let asset_id = registry_data.asset_id();
        if assets_in_pset.contains(&asset_id) {
            let metadata = registry_data.asset_metadata();
            pset.add_asset_metadata(asset_id, &metadata);
            let token_id = registry_data.reissuance_token();
            // TODO: handle blinded issuance
            let issuance_blinded = false;
            pset.add_token_metadata(token_id, &TokenMetadata::new(asset_id, issuance_blinded));
        }
    }
}

/// `RegistryAssetData` contains all the data related to an asset with a contract in the registry.
#[derive(Debug, Clone)]
pub struct RegistryAssetData {
    asset_id: AssetId,
    token_id: AssetId,
    issuance_vin: u32,
    issuance_tx: Transaction,
    contract: Contract,
}

impl RegistryAssetData {
    /// Create a new registry asset data from the asset id, the issuance transaction and the contract
    ///
    /// Returns an error if the issuance transaction is not valid for the given asset id and contract
    pub fn new(
        asset_id: AssetId,
        issuance_tx: Transaction,
        contract: Contract,
    ) -> Result<Self, Error> {
        for (vin, txin) in issuance_tx.input.iter().enumerate() {
            let (asset_id_txin, token_id) = txin.issuance_ids();
            if asset_id_txin == asset_id {
                let (asset_id_contract, token_id_contract) = asset_ids(txin, &contract)?;
                if asset_id_contract != asset_id || token_id_contract != token_id {
                    return Err(Error::InvalidContractForAsset(asset_id.to_string()));
                }
                return Ok(Self {
                    asset_id,
                    token_id,
                    issuance_vin: vin as u32,
                    issuance_tx,
                    contract,
                });
            }
        }
        Err(Error::InvalidIssuanceTxtForAsset(asset_id.to_string()))
    }

    /// Get the contract as a string
    pub fn contract_str(&self) -> String {
        serde_json::to_string(&self.contract).expect("contract")
    }

    /// Get the contract
    pub fn contract(&self) -> &Contract {
        &self.contract
    }

    /// Get the issuance transaction prevout
    pub fn issuance_prevout(&self) -> OutPoint {
        self.issuance_tx.input[self.issuance_vin as usize].previous_output
    }

    /// Get the asset id of the reissuance token of this asset id
    pub fn reissuance_token(&self) -> AssetId {
        self.token_id
    }

    /// Get the token id
    pub fn token_id(&self) -> AssetId {
        self.token_id
    }

    /// Get the asset id
    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// Get the issuance transaction
    pub fn issuance_tx(&self) -> &Transaction {
        &self.issuance_tx
    }

    /// Get the issuance transaction input
    pub fn txin(&self) -> &elements::TxIn {
        &self.issuance_tx.input[self.issuance_vin as usize]
    }

    /// Get the entropy of the issuance transaction
    pub fn entropy(&self) -> Result<[u8; 32], Error> {
        let entropy = AssetId::generate_asset_entropy(
            self.txin().previous_output,
            self.contract.contract_hash()?,
        )
        .to_byte_array();
        Ok(entropy)
    }

    /// Get the asset metadata from this registry asset data
    pub fn asset_metadata(&self) -> AssetMetadata {
        AssetMetadata::new(self.contract_str(), self.issuance_prevout())
    }
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

        let hard_coded_usdt = usdt();
        assert_eq!(hard_coded_usdt.0, tether_asset_id);
        assert_eq!(hard_coded_usdt.1, registry_data);
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

    #[test]
    fn test_registry_cache_hardcoded() {
        let registry = Registry::default_for_network(ElementsNetwork::default_regtest()).unwrap();
        let cache = RegistryCache::new_hardcoded(registry);
        // policy assets of regtest(default)/testnet/mainnet network are hard coded
        let regtest_asset_id = AssetId::from_str(LIQUID_DEFAULT_REGTEST_ASSET_STR).unwrap();
        let testnet_asset_id = AssetId::from_str(LIQUID_TESTNET_POLICY_ASSET_STR).unwrap();
        let mainnet_asset_id = AssetId::from_str(LIQUID_POLICY_ASSET_STR).unwrap();
        let usdt_asset_id =
            AssetId::from_str("ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2")
                .unwrap();
        assert!(cache.get(regtest_asset_id).is_some());
        assert!(cache.get(testnet_asset_id).is_some());
        assert!(cache.get(mainnet_asset_id).is_some());
        assert!(cache.get(usdt_asset_id).is_some());

        let token_id =
            AssetId::from_str("59fe4d2127ba9f16bd6850a3e6271a166e7ed2e1669f6c107d655791c94ee98f")
                .unwrap();
        let asset_id = cache.get_asset_of_token(token_id).unwrap();
        assert_eq!(asset_id.asset_id().unwrap(), usdt_asset_id);
    }

    #[ignore = "require internet connection"]
    #[tokio::test]
    async fn test_registry_cache() {
        let registry = Registry::default_for_network(ElementsNetwork::Liquid).unwrap();
        let asset_id =
            AssetId::from_str("18729918ab4bca843656f08d4dd877bed6641fbd596a0a963abbf199cfeb3cec")
                .unwrap();
        let cache = RegistryCache::new(registry, &[asset_id], 1).await;
        let data = cache.get(asset_id).unwrap();
        assert_eq!(data.contract.ticker, "EURx");
        assert_eq!(data.contract.precision, 8);
        assert_eq!(
            data.contract.contract_hash().unwrap().to_string(),
            "e90594cf35ff894158967d4bec6df0b4f2841818ea5df6a94ca8ef50e9546a27"
        );

        assert_eq!(
            data.issuance_prevout.txid,
            Txid::from_str("fdbeae738138cafedea4931a281f0347c133f1b279f0ef1f09ea2ca898364966")
                .unwrap()
        );
        assert_eq!(data.issuance_prevout.vout, 0);
        assert_eq!(
            data.entropy().unwrap().to_string(),
            "86889dde3fa2fbc8dc75134497be8eaac5e43297f39fa95740626c9c4e9dedf2", // shown on block explorer https://blockstream.info/liquid/tx/a6a340e26ab72139c896c38690489a94e79e580336e9607efde8418f49e6daf7?expand
        );
        assert_eq!(data.asset_id().unwrap().to_string(), asset_id.to_string());
        assert_eq!(
            data.token_id().unwrap().to_string(),
            "e7bf681db0ea93c31cfb4d9d540295ef43d9148835a46d5b286d756852803ff4"
        );

        let registry = Registry::default_for_network(ElementsNetwork::Liquid).unwrap();
        let cache_2 = RegistryCache::new(registry, &[], 1).await;
        assert!(cache_2.get(asset_id).is_none());
    }

    #[test]
    fn test_registry_cache_getters() {
        let registry = Registry::default_for_network(ElementsNetwork::default_regtest()).unwrap();
        let cache = RegistryCache::new_hardcoded(registry);
        let usdt_asset_id =
            AssetId::from_str("ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2")
                .unwrap();
        let data = cache.get(usdt_asset_id).unwrap();
        assert_eq!(data.ticker(), "USDt");
        assert_eq!(data.precision(), 8);
        assert_eq!(data.name(), "Tether USD");
        assert_eq!(data.domain(), "tether.to");
    }

    #[test]
    fn test_add_contracts() {
        let (usdt_asset_id, data) = usdt();
        let usdt_token_id = data.token_id().unwrap();
        let mut pset =
            PartiallySignedTransaction::from_str(lwk_test_util::pset_usdt_no_contracts()).unwrap();
        let registry = Registry::default_for_network(ElementsNetwork::Liquid).unwrap();
        let cache = RegistryCache::new_hardcoded(registry);
        let assets = cache.registry_asset_data();
        assert!(cache.get(usdt_asset_id).is_some());

        assert!(pset.get_asset_metadata(usdt_asset_id).is_none());
        assert!(pset.get_token_metadata(usdt_token_id).is_none());

        add_contracts(&mut pset, assets.iter());

        assert!(pset.get_asset_metadata(usdt_asset_id).is_some());
        assert!(pset.get_token_metadata(usdt_token_id).is_some());

        let pset_with_contract =
            PartiallySignedTransaction::from_str(lwk_test_util::pset_usdt_with_contract()).unwrap();
        assert!(pset_with_contract
            .get_asset_metadata(usdt_asset_id)
            .is_some());
        assert!(pset_with_contract
            .get_token_metadata(usdt_token_id)
            .is_some());
    }
}
