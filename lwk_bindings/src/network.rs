use std::{fmt::Display, sync::Arc};

use lwk_common::electrum_ssl::{LIQUID_SOCKET, LIQUID_TESTNET_SOCKET};

use elements::hex::ToHex;

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

impl From<&Network> for lwk_wollet::ElementsNetwork {
    fn from(value: &Network) -> Self {
        match value.inner {
            lwk_common::Network::Liquid => lwk_wollet::ElementsNetwork::Liquid,
            lwk_common::Network::TestnetLiquid => lwk_wollet::ElementsNetwork::LiquidTestnet,
            lwk_common::Network::CustomElements(_) => {
                lwk_wollet::ElementsNetwork::ElementsRegtest {
                    policy_asset: *value.inner.policy_asset(),
                }
            }
        }
    }
}

impl From<lwk_wollet::ElementsNetwork> for Network {
    fn from(value: lwk_wollet::ElementsNetwork) -> Self {
        match value {
            lwk_wollet::ElementsNetwork::Liquid => lwk_common::Network::Liquid.into(),
            lwk_wollet::ElementsNetwork::LiquidTestnet => lwk_common::Network::TestnetLiquid.into(),
            lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset } => {
                lwk_common::Network::CustomElements(
                    lwk_common::ElementsParamsBuilder::new()
                        .with_policy_asset(policy_asset)
                        .build()
                        .expect("static"),
                )
                .into()
            }
        }
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

    /// Return the default regtest network with the default policy asset
    #[uniffi::constructor]
    pub fn regtest_default() -> Arc<Network> {
        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let policy_asset: elements::AssetId = policy_asset.parse().expect("static");
        Arc::new(
            lwk_common::Network::CustomElements(
                lwk_common::ElementsParamsBuilder::new()
                    .with_policy_asset(policy_asset)
                    .build()
                    .expect("static"),
            )
            .into(),
        )
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
