use crate::AssetId;
use lwk_wollet::elements;
use wasm_bindgen::prelude::*;

/// Wrapper of [`elements::TxOutSecrets`]
#[derive(PartialEq, Eq, Debug)]
#[wasm_bindgen]
pub struct TxOutSecrets {
    inner: elements::TxOutSecrets,
}

impl From<elements::TxOutSecrets> for TxOutSecrets {
    fn from(inner: elements::TxOutSecrets) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl TxOutSecrets {
    pub fn asset(&self) -> AssetId {
        self.inner.asset.into()
    }

    #[wasm_bindgen(js_name = assetBlindingFactor)]
    pub fn asset_blinding_factor(&self) -> String {
        self.inner
            .asset_bf
            .to_string()
            .parse()
            .expect("asset_bf to_string creates valid hex")
    }

    pub fn value(&self) -> u64 {
        self.inner.value
    }

    #[wasm_bindgen(js_name = valueBlindingFactor)]
    pub fn value_blinding_factor(&self) -> String {
        self.inner
            .value_bf
            .to_string()
            .parse()
            .expect("value_bf to_string creates valid hex")
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use lwk_wollet::elements;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn tx_out_secrets() {
        // TODO use abf and vbf different from zero
        let elements_tx_out_secrets = elements::TxOutSecrets::new(
            elements::AssetId::default(),
            elements::confidential::AssetBlindingFactor::zero(),
            1000,
            elements::confidential::ValueBlindingFactor::zero(),
        );
        let tx_out_secrets: crate::TxOutSecrets = elements_tx_out_secrets.into();

        assert_eq!(tx_out_secrets.value(), 1000);
        assert_eq!(
            tx_out_secrets.value_blinding_factor().to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );

        assert_eq!(
            tx_out_secrets.asset().to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(
            tx_out_secrets.asset_blinding_factor().to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }
}
