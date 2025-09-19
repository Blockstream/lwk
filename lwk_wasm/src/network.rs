use lwk_wollet::elements;
use wasm_bindgen::prelude::*;

use crate::{AssetId, EsploraClient, TxBuilder};

/// The network of the elements blockchain such as mainnet, testnet or regtest.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct Network {
    inner: lwk_wollet::ElementsNetwork,
}

impl std::fmt::Display for Network {
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

impl From<Network> for lwk_common::Network {
    fn from(value: Network) -> Self {
        match value.inner {
            lwk_wollet::ElementsNetwork::Liquid => lwk_common::Network::Liquid,
            lwk_wollet::ElementsNetwork::LiquidTestnet => lwk_common::Network::TestnetLiquid,
            lwk_wollet::ElementsNetwork::ElementsRegtest { .. } => {
                lwk_common::Network::LocaltestLiquid
            }
        }
    }
}

#[wasm_bindgen]
impl Network {
    /// Creates a mainnet `Network``
    pub fn mainnet() -> Network {
        lwk_wollet::ElementsNetwork::Liquid.into()
    }

    /// Creates a testnet `Network``
    pub fn testnet() -> Network {
        lwk_wollet::ElementsNetwork::LiquidTestnet.into()
    }

    /// Creates a regtest `Network``
    pub fn regtest(policy_asset: &AssetId) -> Network {
        lwk_wollet::ElementsNetwork::ElementsRegtest {
            policy_asset: (*policy_asset).into(),
        }
        .into()
    }

    /// Creates the default regtest `Network` with the policy asset `5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225`
    #[wasm_bindgen(js_name = regtestDefault)]
    pub fn regtest_default() -> Network {
        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let policy_asset: elements::AssetId = policy_asset.parse().expect("static");
        lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset }.into()
    }

    #[wasm_bindgen(js_name = defaultEsploraClient)]
    pub fn default_esplora_client(&self) -> EsploraClient {
        let url = match &self.inner {
            lwk_wollet::ElementsNetwork::Liquid => "https://blockstream.info/liquid/api",
            lwk_wollet::ElementsNetwork::LiquidTestnet => {
                "https://blockstream.info/liquidtestnet/api"
            }
            lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ } => "127.0.0.1:3000",
        };

        EsploraClient::new(self, url, false, 1, false).unwrap()
    }

    #[wasm_bindgen(js_name = isMainnet)]
    pub fn is_mainnet(&self) -> bool {
        matches!(&self.inner, &lwk_wollet::ElementsNetwork::Liquid)
    }

    #[wasm_bindgen(js_name = isTestnet)]
    pub fn is_testnet(&self) -> bool {
        matches!(&self.inner, &lwk_wollet::ElementsNetwork::LiquidTestnet)
    }

    #[wasm_bindgen(js_name = isRegtest)]
    pub fn is_regtest(&self) -> bool {
        matches!(
            &self.inner,
            &lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ }
        )
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }

    #[wasm_bindgen(js_name = policyAsset)]
    pub fn policy_asset(&self) -> AssetId {
        self.inner.policy_asset().into()
    }

    #[wasm_bindgen(js_name = txBuilder)]
    pub fn tx_builder(&self) -> TxBuilder {
        TxBuilder::new(self)
    }

    #[wasm_bindgen(js_name = defaultExplorerUrl)]
    pub fn default_explorer_url(&self) -> String {
        let url = match &self.inner {
            lwk_wollet::ElementsNetwork::Liquid => "https://blockstream.info/liquid/",
            lwk_wollet::ElementsNetwork::LiquidTestnet => "https://blockstream.info/liquidtestnet/",
            lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ } => "127.0.0.1:3000",
        };
        url.to_string()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use wasm_bindgen_test::*;

    use crate::Network;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_network() {
        assert_eq!(Network::mainnet().to_string(), "Liquid");
        assert_eq!(Network::testnet().to_string(), "LiquidTestnet");
        assert_eq!(Network::regtest_default().to_string(), "ElementsRegtest { policy_asset: 5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225 }");
    }
}
