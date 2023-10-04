use crate::domain::verify_domain_name;
use crate::elements::hashes::{sha256, Hash};
use crate::elements::ContractHash;
use crate::error::Error;
use crate::util::{serde_from_hex, serde_to_hex, verify_pubkey};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

static RE_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[[:ascii:]]{1,255}$").unwrap());
static RE_TICKER: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z0-9.\-]{3,24}$").unwrap());

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Entity {
    #[serde(rename = "domain")]
    Domain(String),
}

// TODO: should we allow the caller to set extra arbitrary fields? if so how should we treat them?
// should we allow them to contribute to the contract hash, but we should skip validation for
// those? For instance how should we handle the nonce field that asset
// 123465c803ae336c62180e52d94ee80d80828db54df9bedbb9860060f49de2eb has?

// Order of the fields here determines the serialization order, make sure it's ordered
// lexicographically.
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

        contract.issuer_pubkey = vec![2];
        assert!(contract.validate().is_err());
        contract.issuer_pubkey = vec![2; 32];

        contract.ticker = "US".to_string();
        assert!(contract.validate().is_err());
        contract.ticker = "USDt".to_string();

        contract.name = "Tether USDü".to_string();
        assert!(contract.validate().is_err());
        contract.name = "Tether USD".to_string();

        contract.precision = 9;
        assert!(contract.validate().is_err());
        contract.precision = 8;

        contract.version = 1;
        assert!(contract.validate().is_err());
        contract.version = 0;
    }
}
