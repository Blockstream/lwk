//! Registry related functions
//!
//! The Registry is repository to store and retrieve asset metadata, like the name or the ticker of an asset.

use std::str::FromStr;

use crate::domain::verify_domain_name;
use crate::elements::hashes::{sha256, Hash};
use crate::elements::{AssetId, ContractHash, OutPoint};
use crate::error::Error;
use crate::util::{serde_from_hex, serde_to_hex, verify_pubkey};
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

#[cfg(test)]
mod tests {
    use super::*;
    use elements::hex::FromHex;
    use std::str::FromStr;

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
}
