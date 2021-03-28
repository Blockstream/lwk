use crate::error::Error;

use bitcoin::hashes::hex::FromHex;

// TODO: policy asset should only be set for ElementsRegtest, fail otherwise
const LIQUID_POLICY_ASSET_STR: &str =
    "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

#[derive(Debug, Clone)]
pub struct Config {
    network: ElementsNetwork,
    policy_asset: elements::issuance::AssetId,

    pub tls: bool,
    pub validate_domain: bool,
    pub spv_enabled: bool,
    pub electrum_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementsNetwork {
    Liquid,
    ElementsRegtest,
}

impl Config {
    pub fn new_regtest(
        tls: bool,
        validate_domain: bool,
        spv_enabled: bool,
        electrum_url: &str,
        policy_asset: &str,
    ) -> Result<Self, Error> {
        Ok(Config {
            network: ElementsNetwork::ElementsRegtest,
            tls,
            validate_domain,
            spv_enabled,
            electrum_url: Some(electrum_url.to_string()),
            policy_asset: elements::issuance::AssetId::from_hex(policy_asset)?,
        })
    }

    pub fn new_mainnet(
        tls: bool,
        validate_domain: bool,
        spv_enabled: bool,
        electrum_url: &str,
    ) -> Result<Self, Error> {
        Ok(Config {
            network: ElementsNetwork::Liquid,
            tls,
            validate_domain,
            spv_enabled,
            electrum_url: Some(electrum_url.to_string()),
            policy_asset: elements::issuance::AssetId::from_hex(LIQUID_POLICY_ASSET_STR)?,
        })
    }

    pub fn network(&self) -> ElementsNetwork {
        self.network
    }

    pub fn policy_asset(&self) -> elements::issuance::AssetId {
        self.policy_asset
    }
}
