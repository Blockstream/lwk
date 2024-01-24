use std::{fmt::Display, sync::Arc};

use crate::{types::AssetId, ElectrumClient, LwkError};

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

#[uniffi::export]
impl Network {
    pub fn default_electrum_client(&self) -> Result<Arc<ElectrumClient>, LwkError> {
        let (url, validate_domain, tls) = match &self.inner {
            lwk_wollet::ElementsNetwork::Liquid => ("blockstream.info:995", true, true),
            lwk_wollet::ElementsNetwork::LiquidTestnet => ("blockstream.info:465", true, true),
            lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ } => {
                ("127.0.0.1:50002", false, false)
            }
        };

        ElectrumClient::new(url, tls, validate_domain)
    }
}

#[uniffi::export]
impl Network {
    pub fn is_mainnet(&self) -> bool {
        matches!(&self.inner, &lwk_wollet::ElementsNetwork::Liquid)
    }
}

#[derive(uniffi::Object)]
pub struct NetworkBuilder {}

// TODO remove NetworkBuilder, while you can't have associated function you can have more than one constructor!
#[uniffi::export]
impl NetworkBuilder {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }

    pub fn mainnet(&self) -> Arc<Network> {
        Arc::new(lwk_wollet::ElementsNetwork::Liquid.into())
    }

    pub fn testnet(&self) -> Arc<Network> {
        Arc::new(lwk_wollet::ElementsNetwork::LiquidTestnet.into())
    }

    pub fn regtest(&self, policy_asset: AssetId) -> Arc<Network> {
        Arc::new(
            lwk_wollet::ElementsNetwork::ElementsRegtest {
                policy_asset: policy_asset.into(),
            }
            .into(),
        )
    }

    pub fn regtest_default(&self) -> Arc<Network> {
        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let policy_asset: elements::AssetId = policy_asset.parse().expect("static");
        Arc::new(lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset }.into())
    }
}
