use crate::elements::{AddressParams, AssetId};
use crate::error::Error;
use std::str::FromStr;

const LIQUID_POLICY_ASSET_STR: &str =
    "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
const LIQUID_TESTNET_POLICY_ASSET_STR: &str =
    "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ElementsNetwork {
    Liquid,
    LiquidTestnet,
    ElementsRegtest { policy_asset: AssetId },
}

impl ElementsNetwork {
    pub fn policy_asset(&self) -> AssetId {
        match self {
            ElementsNetwork::Liquid => {
                AssetId::from_str(LIQUID_POLICY_ASSET_STR).expect("can't fail on const")
            }
            ElementsNetwork::LiquidTestnet => {
                AssetId::from_str(LIQUID_TESTNET_POLICY_ASSET_STR).expect("can't fail on const")
            }
            ElementsNetwork::ElementsRegtest { policy_asset } => *policy_asset,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ElementsNetwork::Liquid => "liquid",
            ElementsNetwork::LiquidTestnet => "liquid-testnet",
            ElementsNetwork::ElementsRegtest { .. } => "liquid-regtest",
        }
    }

    pub fn address_params(&self) -> &'static AddressParams {
        match self {
            ElementsNetwork::Liquid => &AddressParams::LIQUID,
            ElementsNetwork::LiquidTestnet => &AddressParams::LIQUID_TESTNET,
            ElementsNetwork::ElementsRegtest { .. } => &AddressParams::ELEMENTS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    network: ElementsNetwork,
}

impl Config {
    pub fn new(network: ElementsNetwork) -> Result<Self, Error> {
        Ok(Config { network })
    }

    pub fn address_params(&self) -> &'static AddressParams {
        self.network.address_params()
    }

    pub fn policy_asset(&self) -> AssetId {
        self.network.policy_asset()
    }

    pub fn network(&self) -> &ElementsNetwork {
        &self.network
    }
}
