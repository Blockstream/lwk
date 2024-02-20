use lwk_wollet::elements;
use wasm_bindgen::prelude::*;

use crate::{AssetId, EsploraClient};

#[wasm_bindgen]
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

#[wasm_bindgen]
impl Network {
    pub fn mainnet() -> Network {
        lwk_wollet::ElementsNetwork::Liquid.into()
    }

    pub fn testnet() -> Network {
        lwk_wollet::ElementsNetwork::LiquidTestnet.into()
    }

    pub fn regtest(policy_asset: AssetId) -> Network {
        lwk_wollet::ElementsNetwork::ElementsRegtest {
            policy_asset: policy_asset.into(),
        }
        .into()
    }

    pub fn regtest_default() -> Network {
        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let policy_asset: elements::AssetId = policy_asset.parse().expect("static");
        lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset }.into()
    }

    pub fn default_esplora_client(&self) -> EsploraClient {
        let url = match &self.inner {
            lwk_wollet::ElementsNetwork::Liquid => "https://blockstream.info/liquid/api",
            lwk_wollet::ElementsNetwork::LiquidTestnet => {
                "https://blockstream.info/liquidtestnet/api"
            }
            lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset: _ } => "127.0.0.1:3000",
        };

        EsploraClient::new(url)
    }

    pub fn is_mainnet(&self) -> bool {
        matches!(&self.inner, &lwk_wollet::ElementsNetwork::Liquid)
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }
}

#[cfg(test)]
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
