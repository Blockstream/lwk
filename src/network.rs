use crate::error::Error;
use crate::transaction::asset_to_bin;
use crate::transaction::AssetId;
use elements::confidential::Asset;
use elements::{confidential, issuance};
use serde::{Deserialize, Serialize};

// TODO: policy asset should only be set for ElementsRegtest, fail otherwise
const LIQUID_POLICY_ASSET_STR: &str =
    "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub development: bool,
    pub liquid: bool,
    pub mainnet: bool,

    pub tls: Option<bool>,
    pub electrum_url: Option<String>,
    pub validate_domain: Option<bool>,
    pub policy_asset: Option<String>,
    pub ct_bits: Option<i32>,
    pub ct_exponent: Option<i32>,
    pub spv_enabled: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementsNetwork {
    Liquid,
    ElementsRegtest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkId {
    Elements(ElementsNetwork),
    Bitcoin(bitcoin::Network),
}

impl Config {
    pub fn network_id(&self) -> NetworkId {
        match (self.liquid, self.mainnet, self.development) {
            (true, true, false) => NetworkId::Elements(ElementsNetwork::Liquid),
            (true, false, true) => NetworkId::Elements(ElementsNetwork::ElementsRegtest),
            (false, true, false) => NetworkId::Bitcoin(bitcoin::Network::Bitcoin),
            (false, false, false) => NetworkId::Bitcoin(bitcoin::Network::Testnet),
            (false, false, true) => NetworkId::Bitcoin(bitcoin::Network::Regtest),
            (l, m, d) => panic!(
                "inconsistent network parameters: lq={}, main={}, dev={}",
                l, m, d
            ),
        }
    }

    pub fn policy_asset_id(&self) -> Result<AssetId, Error> {
        if self.liquid {
            if self.development {
                match self.policy_asset.as_ref() {
                    Some(policy_asset_str) => Ok(asset_to_bin(policy_asset_str)?),
                    None => Err("no policy asset".into()),
                }
            } else {
                Ok(asset_to_bin(LIQUID_POLICY_ASSET_STR)?)
            }
        } else {
            Err("no policy asset".into())
        }
    }

    pub fn policy_asset(&self) -> Result<Asset, Error> {
        let asset_id = self.policy_asset_id()?;
        let asset_id = issuance::AssetId::from_slice(&asset_id)?;
        Ok(confidential::Asset::Explicit(asset_id))
    }
}
