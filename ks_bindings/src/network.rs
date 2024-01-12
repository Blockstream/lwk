use std::{fmt::Display, sync::Arc};

use crate::{electrum_url::ElectrumUrl, types::AssetId};

#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
#[uniffi::export(Display)]
pub struct ElementsNetwork {
    inner: wollet::ElementsNetwork,
}

impl Display for ElementsNetwork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}
impl From<wollet::ElementsNetwork> for ElementsNetwork {
    fn from(inner: wollet::ElementsNetwork) -> Self {
        Self { inner }
    }
}

impl From<ElementsNetwork> for wollet::ElementsNetwork {
    fn from(value: ElementsNetwork) -> Self {
        value.inner
    }
}

#[uniffi::export]
impl ElementsNetwork {
    pub fn default_electrum_url(&self) -> Arc<ElectrumUrl> {
        let (url, validate_domain, tls) = match &self.inner {
            wollet::ElementsNetwork::Liquid => ("blockstream.info:995", true, true),
            wollet::ElementsNetwork::LiquidTestnet => ("blockstream.info:465", true, true),
            wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ } => {
                ("127.0.0.1:50002", false, false)
            }
        };

        Arc::new(ElectrumUrl {
            url: url.to_string(),
            tls,
            validate_domain,
        })
    }
}

#[uniffi::export]
impl ElementsNetwork {
    pub fn is_mainnet(&self) -> bool {
        matches!(&self.inner, &wollet::ElementsNetwork::Liquid)
    }
}

#[uniffi::export]
fn new_mainnet_network() -> Arc<ElementsNetwork> {
    Arc::new(wollet::ElementsNetwork::Liquid.into())
}

#[uniffi::export]
fn new_testnet_network() -> Arc<ElementsNetwork> {
    Arc::new(wollet::ElementsNetwork::LiquidTestnet.into())
}

#[uniffi::export]
fn new_regtest_network(policy_asset: AssetId) -> Arc<ElementsNetwork> {
    Arc::new(
        wollet::ElementsNetwork::ElementsRegtest {
            policy_asset: policy_asset.into(),
        }
        .into(),
    )
}

#[uniffi::export]
fn new_default_regtest_network() -> Arc<ElementsNetwork> {
    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let policy_asset: elements::AssetId = policy_asset.parse().expect("static");
    Arc::new(wollet::ElementsNetwork::ElementsRegtest { policy_asset }.into())
}
