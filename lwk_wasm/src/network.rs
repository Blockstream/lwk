use wasm_bindgen::prelude::*;

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

    pub fn is_mainnet(&self) -> bool {
        matches!(&self.inner, &lwk_wollet::ElementsNetwork::Liquid)
    }
}

mod tests {

    use wasm_bindgen_test::*;

    use crate::Network;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_network() {
        assert_eq!(Network::mainnet().to_string(), "Liquid");
        assert_eq!(Network::testnet().to_string(), "LiquidTestnet");
    }
}
