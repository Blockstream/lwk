use std::fmt::Display;

use wasm_bindgen::prelude::*;

use crate::Network;

#[wasm_bindgen]
#[derive(Debug)]
pub struct TxBuilder {
    inner: lwk_wollet::TxBuilder,
}

#[wasm_bindgen]
impl TxBuilder {
    /// Creates a transaction builder
    #[wasm_bindgen(constructor)]
    pub fn new(network: Network) -> TxBuilder {
        TxBuilder {
            inner: lwk_wollet::TxBuilder::new(network.into()),
        }
    }

    pub fn fee_rate(self, fee_rate: Option<f32>) -> TxBuilder {
        TxBuilder {
            inner: self.inner.fee_rate(fee_rate),
        }
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

impl Display for TxBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use crate::Network;

    use super::TxBuilder;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_builder() {
        let network = Network::mainnet();
        let mut builder = TxBuilder::new(network);
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, addressees: [], fee_rate: 100.0, issuance_request: None }");
        builder = builder.fee_rate(Some(200.0));
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, addressees: [], fee_rate: 200.0, issuance_request: None }");
    }
}
