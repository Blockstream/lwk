use crate::error::Error;
use serde::{Deserialize, Serialize};

use bitcoin::hashes::hex::FromHex;

// TODO: policy asset should only be set for ElementsRegtest, fail otherwise
const LIQUID_POLICY_ASSET_STR: &str =
    "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub mainnet: bool,

    pub tls: bool,
    pub validate_domain: bool,
    pub spv_enabled: bool,
    pub electrum_url: Option<String>,
    pub policy_asset: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementsNetwork {
    Liquid,
    ElementsRegtest,
}

impl Config {
    pub fn network(&self) -> ElementsNetwork {
        match self.mainnet {
            true => ElementsNetwork::Liquid,
            false => ElementsNetwork::ElementsRegtest,
        }
    }

    pub fn policy_asset_id(&self) -> Result<elements::issuance::AssetId, Error> {
        match self.network() {
            ElementsNetwork::Liquid => Ok(elements::issuance::AssetId::from_hex(
                LIQUID_POLICY_ASSET_STR,
            )?),
            ElementsNetwork::ElementsRegtest => {
                // TODO: pack policy asset in ElementsRegtest variant
                //let asset_str = self.policy_asset.as_ref().unwrap_or_else(|| Err("no policy_asset".into()));
                match self.policy_asset.as_ref() {
                    Some(policy_asset_str) => {
                        Ok(elements::issuance::AssetId::from_hex(policy_asset_str)?)
                    }
                    None => Err("no policy asset".into()),
                }
            }
        }
    }
}
