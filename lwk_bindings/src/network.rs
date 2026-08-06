use std::{fmt::Display, str::FromStr, sync::Arc};

use lwk_common::electrum_ssl::{LIQUID_SOCKET, LIQUID_TESTNET_SOCKET};

use elements::{hex::ToHex, BlockHash};

use crate::{types::AssetId, ElectrumClient, EsploraClient, LwkError, TxBuilder};

/// The network of the elements blockchain.
#[derive(uniffi::Object, PartialEq, Eq, Hash, Debug, Clone, Copy)]
#[uniffi::export(Display, Hash, Eq)]
pub struct Network {
    pub(crate) inner: lwk_common::Network,
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            lwk_common::Network::Liquid => write!(f, "Liquid"),
            lwk_common::Network::TestnetLiquid => write!(f, "LiquidTestnet"),
            lwk_common::Network::CustomElements(_) => write!(f, "{:?}", &self.inner),
        }
    }
}
impl From<lwk_common::Network> for Network {
    fn from(inner: lwk_common::Network) -> Self {
        Self { inner }
    }
}

impl From<Network> for lwk_common::Network {
    fn from(value: Network) -> Self {
        value.inner
    }
}

impl From<&Network> for lwk_common::Network {
    fn from(value: &Network) -> Self {
        value.inner
    }
}

#[uniffi::export]
impl Network {
    /// Return the mainnet network
    #[uniffi::constructor]
    pub fn mainnet() -> Arc<Network> {
        Arc::new(lwk_common::Network::Liquid.into())
    }

    /// Return the testnet network
    #[uniffi::constructor]
    pub fn testnet() -> Arc<Network> {
        Arc::new(lwk_common::Network::TestnetLiquid.into())
    }

    /// Return the regtest network with the given policy asset
    #[uniffi::constructor]
    pub fn regtest(policy_asset: AssetId) -> Arc<Network> {
        Arc::new(
            lwk_common::Network::CustomElements(
                lwk_common::ElementsParamsBuilder::new()
                    .with_policy_asset(policy_asset.into())
                    .build()
                    .expect("static"),
            )
            .into(),
        )
    }

    /// Return a custom Elements network with the given policy asset and genesis block hash.
    ///
    /// The genesis block hash uses the conventional display-order hexadecimal encoding returned
    /// by Elements RPCs such as `getblockhash 0`.
    #[uniffi::constructor]
    pub fn custom(policy_asset: AssetId, genesis_hash: &str) -> Result<Arc<Network>, LwkError> {
        let genesis_hash = BlockHash::from_str(genesis_hash).map_err(|e| LwkError::Generic {
            msg: format!("invalid genesis block hash: {e}"),
        })?;
        let params = lwk_common::ElementsParamsBuilder::new()
            .with_policy_asset(policy_asset.into())
            .with_genesis_hash(genesis_hash)
            .build()
            .map_err(|e| LwkError::Generic { msg: e.to_string() })?;

        Ok(Arc::new(lwk_common::Network::CustomElements(params).into()))
    }

    /// Return the default regtest network with the default policy asset
    #[uniffi::constructor]
    pub fn regtest_default() -> Arc<Network> {
        Arc::new(lwk_common::Network::default_regtest().into())
    }

    /// Return the default electrum client for this network
    pub fn default_electrum_client(&self) -> Result<Arc<ElectrumClient>, LwkError> {
        let (url, validate_domain, tls) = match &self.inner {
            lwk_common::Network::Liquid => (LIQUID_SOCKET, true, true),
            lwk_common::Network::TestnetLiquid => (LIQUID_TESTNET_SOCKET, true, true),
            lwk_common::Network::CustomElements(_) => ("127.0.0.1:50002", false, false),
        };

        ElectrumClient::new(url, tls, validate_domain)
    }

    /// Return the default esplora client for this network
    pub fn default_esplora_client(&self) -> Result<Arc<EsploraClient>, LwkError> {
        let url = match &self.inner {
            lwk_common::Network::Liquid => "https://blockstream.info/liquid/api",
            lwk_common::Network::TestnetLiquid => "https://blockstream.info/liquidtestnet/api",
            lwk_common::Network::CustomElements(_) => "127.0.0.1:3000",
        };

        EsploraClient::new(url, &self.inner.into())
    }

    /// Return true if the network is the mainnet network
    pub fn is_mainnet(&self) -> bool {
        matches!(&self.inner, &lwk_common::Network::Liquid)
    }

    /// Return the policy asset (eg LBTC for mainnet) for this network
    pub fn policy_asset(&self) -> AssetId {
        (*self.inner.policy_asset()).into()
    }

    /// Return the genesis block hash for this network as hex string.
    pub fn genesis_block_hash(&self) -> String {
        self.inner.genesis_hash().to_hex()
    }

    /// Return a new `TxBuilder` for this network
    pub fn tx_builder(&self) -> Arc<TxBuilder> {
        Arc::new(TxBuilder::new(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ELEMENTS_REGTEST_GENESIS: &str =
        "cd179c84c35f51825f20a3b91a18d45f0c53b5ceb744a5b6ef8f0babe809396f";
    const ELEMENTS_REGTEST_POLICY_ASSET: &str =
        "b2e15d0d7a0c94e4e2ce0fe6e8691b9e451377f6e46e8045a86f7c4b5d4f0f23";

    fn elements_regtest_policy_asset() -> AssetId {
        elements::AssetId::from_str(ELEMENTS_REGTEST_POLICY_ASSET)
            .expect("valid asset id")
            .into()
    }

    #[test]
    fn custom_network_preserves_explicit_identity() {
        let network = Network::custom(elements_regtest_policy_asset(), ELEMENTS_REGTEST_GENESIS)
            .expect("valid custom network");

        assert_eq!(
            network.policy_asset().to_string(),
            ELEMENTS_REGTEST_POLICY_ASSET
        );
        assert_eq!(network.genesis_block_hash(), ELEMENTS_REGTEST_GENESIS);
        assert!(!network.is_mainnet());
    }

    #[test]
    fn custom_network_rejects_invalid_genesis() {
        assert!(Network::custom(elements_regtest_policy_asset(), "00").is_err());
        assert!(Network::custom(
            elements_regtest_policy_asset(),
            "zz179c84c35f51825f20a3b91a18d45f0c53b5ceb744a5b6ef8f0babe809396f",
        )
        .is_err());
    }
}
