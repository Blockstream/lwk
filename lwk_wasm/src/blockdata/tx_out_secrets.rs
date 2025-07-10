use crate::AssetId;
use lwk_wollet::elements;
use lwk_wollet::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
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

    #[wasm_bindgen(js_name = isExplicit)]
    pub fn is_explicit(&self) -> bool {
        self.inner.asset_bf == AssetBlindingFactor::zero()
            && self.inner.value_bf == ValueBlindingFactor::zero()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use elements::AssetId;
    use lwk_wollet::elements;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn tx_out_secrets() {
        let zero_hex = "0000000000000000000000000000000000000000000000000000000000000000";
        let asset_hex = "1111111111111111111111111111111111111111111111111111111111111111";

        // TODO use abf and vbf different from zero
        let txoutsecrets_explicit: crate::TxOutSecrets = elements::TxOutSecrets::new(
            AssetId::from_str(asset_hex).unwrap(),
            AssetBlindingFactor::zero(),
            1000,
            ValueBlindingFactor::zero(),
        )
        .into();

        assert!(txoutsecrets_explicit.is_explicit());
        assert_eq!(txoutsecrets_explicit.value(), 1000);
        assert_eq!(txoutsecrets_explicit.asset().to_string(), asset_hex);
        assert_eq!(
            txoutsecrets_explicit.value_blinding_factor().to_string(),
            zero_hex
        );
        assert_eq!(
            txoutsecrets_explicit.asset_blinding_factor().to_string(),
            zero_hex
        );
    }
}
