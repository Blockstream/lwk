use crate::error::Error;
use elements::bitcoin::hashes::hex::FromHex;
use electrum_client::{Client, ConfigBuilder}; 
use elements::{AddressParams, AssetId};

// TODO: policy asset should only be set for ElementsRegtest, fail otherwise
const LIQUID_POLICY_ASSET_STR: &str =
    "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
const LIQUID_TESTNET_POLICY_ASSET_STR: &str =
    "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";

#[derive(Debug, Clone)]
pub enum ElectrumUrl {
    Tls(String, bool), // the bool value indicates if the domain name should be validated
    Plaintext(String),
}

impl ElectrumUrl {
    pub fn build_client(&self) -> Result<Client, Error> {
        let builder = ConfigBuilder::new();
        let (url, builder) = match self {
            ElectrumUrl::Tls(url, validate) => {
                (format!("ssl://{}", url), builder.validate_domain(*validate))
            }
            ElectrumUrl::Plaintext(url) => (format!("tcp://{}", url), builder),
        };
        Ok(Client::from_config(&url, builder.build())?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementsNetwork {
    Liquid,
    LiquidTestnet,
    ElementsRegtest,
}

impl ElementsNetwork {
    pub fn address_params(&self) -> &'static AddressParams {
        match self {
            ElementsNetwork::Liquid => &AddressParams::LIQUID,
            ElementsNetwork::LiquidTestnet => &AddressParams::LIQUID_TESTNET,
            ElementsNetwork::ElementsRegtest => &AddressParams::ELEMENTS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    network: ElementsNetwork,
    policy_asset: AssetId,
    electrum_url: ElectrumUrl,
}

impl Config {
    pub fn new_regtest(
        tls: bool,
        validate_domain: bool,
        electrum_url: &str,
        policy_asset: &str,
    ) -> Result<Self, Error> {
        let electrum_url = match tls {
            true => ElectrumUrl::Tls(electrum_url.into(), validate_domain),
            false => ElectrumUrl::Plaintext(electrum_url.into()),
        };
        Ok(Config {
            network: ElementsNetwork::ElementsRegtest,
            electrum_url,
            policy_asset: AssetId::from_hex(policy_asset)?,
        })
    }

    pub fn new_testnet(
        tls: bool,
        validate_domain: bool,
        electrum_url: &str,
    ) -> Result<Self, Error> {
        let electrum_url = match tls {
            true => ElectrumUrl::Tls(electrum_url.into(), validate_domain),
            false => ElectrumUrl::Plaintext(electrum_url.into()),
        };
        Ok(Config {
            network: ElementsNetwork::LiquidTestnet,
            electrum_url,
            policy_asset: AssetId::from_hex(LIQUID_TESTNET_POLICY_ASSET_STR)?,
        })
    }

    pub fn new_mainnet(
        tls: bool,
        validate_domain: bool,
        electrum_url: &str,
    ) -> Result<Self, Error> {
        let electrum_url = match tls {
            true => ElectrumUrl::Tls(electrum_url.into(), validate_domain),
            false => ElectrumUrl::Plaintext(electrum_url.into()),
        };
        Ok(Config {
            network: ElementsNetwork::Liquid,
            electrum_url,
            policy_asset: AssetId::from_hex(LIQUID_POLICY_ASSET_STR)?,
        })
    }

    pub fn network(&self) -> ElementsNetwork {
        self.network
    }

    pub fn policy_asset(&self) -> AssetId {
        self.policy_asset
    }

    pub fn electrum_url(&self) -> ElectrumUrl {
        self.electrum_url.clone()
    }
}
