use std::{fmt::Display, sync::Arc};

use lwk_common::electrum_ssl::{LIQUID_SOCKET, LIQUID_TESTNET_SOCKET};

use crate::{types::AssetId, ElectrumClient, EsploraClient, LwkError, TxBuilder};

/// Wrapper over [`lwk_wollet::ElementsNetwork`]
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
#[uniffi::export(Display)]
pub struct Network {
    inner: lwk_wollet::ElementsNetwork,
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}
impl From<lwk_wollet::ElementsNetwork> for Network {
    fn from(inner: lwk_wollet::ElementsNetwork) -> Self {
        Self { inner }
    }
}

impl From<Network> for lwk_wollet::ElementsNetwork {
    fn from(value: Network) -> Self {
        value.inner
    }
}

impl From<&Network> for lwk_wollet::ElementsNetwork {
    fn from(value: &Network) -> Self {
        value.inner
    }
}

#[uniffi::export]
impl Network {
    #[uniffi::constructor]
    pub fn mainnet() -> Arc<Network> {
        Arc::new(lwk_wollet::ElementsNetwork::Liquid.into())
    }

    #[uniffi::constructor]
    pub fn testnet() -> Arc<Network> {
        Arc::new(lwk_wollet::ElementsNetwork::LiquidTestnet.into())
    }

    #[uniffi::constructor]
    pub fn regtest(policy_asset: AssetId) -> Arc<Network> {
        Arc::new(
            lwk_wollet::ElementsNetwork::ElementsRegtest {
                policy_asset: policy_asset.into(),
            }
            .into(),
        )
    }

    #[uniffi::constructor]
    pub fn regtest_default() -> Arc<Network> {
        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let policy_asset: elements::AssetId = policy_asset.parse().expect("static");
        Arc::new(lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset }.into())
    }

    pub fn default_electrum_client(&self) -> Result<Arc<ElectrumClient>, LwkError> {
        let (url, validate_domain, tls) = match &self.inner {
            lwk_wollet::ElementsNetwork::Liquid => (LIQUID_SOCKET, true, true),
            lwk_wollet::ElementsNetwork::LiquidTestnet => (LIQUID_TESTNET_SOCKET, true, true),
            lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ } => {
                ("127.0.0.1:50002", false, false)
            }
        };

        ElectrumClient::new(url, tls, validate_domain)
    }

    pub fn default_esplora_client(&self) -> Result<Arc<EsploraClient>, LwkError> {
        let url = match &self.inner {
            lwk_wollet::ElementsNetwork::Liquid => "https://blockstream.info/liquid/api",
            lwk_wollet::ElementsNetwork::LiquidTestnet => {
                "https://blockstream.info/liquidtestnet/api"
            }
            lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ } => "127.0.0.1:3000",
        };

        EsploraClient::new(url, &self.inner.into())
    }

    pub fn is_mainnet(&self) -> bool {
        matches!(&self.inner, &lwk_wollet::ElementsNetwork::Liquid)
    }

    pub fn policy_asset(&self) -> AssetId {
        self.inner.policy_asset().into()
    }

    pub fn tx_builder(&self) -> Arc<TxBuilder> {
        Arc::new(TxBuilder::new(self))
    }
}
