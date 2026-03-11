use crate::blockdata::blinding_factor;
use crate::AssetId;

use lwk_wollet::elements;
use lwk_wollet::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use lwk_wollet::elements::secp256k1_zkp::{Generator, PedersenCommitment, Tag};
use lwk_wollet::EC;

use wasm_bindgen::prelude::*;

/// Contains unblinded information such as the asset and the value of a transaction output
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

impl From<&TxOutSecrets> for elements::TxOutSecrets {
    fn from(value: &TxOutSecrets) -> Self {
        value.inner
    }
}

#[cfg(feature = "simplicity")]
#[wasm_bindgen]
impl TxOutSecrets {
    /// Creates a new `TxOutSecrets` with the given asset, blinding factors, and value.
    #[wasm_bindgen(constructor)]
    pub fn new(
        asset_id: &AssetId,
        asset_bf: &blinding_factor::AssetBlindingFactor,
        value: u64,
        value_bf: &blinding_factor::ValueBlindingFactor,
    ) -> TxOutSecrets {
        TxOutSecrets {
            inner: elements::TxOutSecrets::new(
                asset_id.into(),
                asset_bf.into(),
                value,
                value_bf.into(),
            ),
        }
    }
}

#[wasm_bindgen]
impl TxOutSecrets {
    /// Creates a new `TxOutSecrets` for an explicit (unblinded) output.
    ///
    /// The blinding factors are set to zero.
    #[wasm_bindgen(js_name = fromExplicit)]
    pub fn from_explicit(asset_id: &AssetId, value: u64) -> TxOutSecrets {
        TxOutSecrets {
            inner: elements::TxOutSecrets::new(
                asset_id.into(),
                AssetBlindingFactor::zero(),
                value,
                ValueBlindingFactor::zero(),
            ),
        }
    }

    /// Return the asset of the output.
    pub fn asset(&self) -> AssetId {
        self.inner.asset.into()
    }

    /// Return the asset blinding factor as a typed object.
    #[wasm_bindgen(js_name = assetBlindingFactor)]
    pub fn asset_blinding_factor(&self) -> blinding_factor::AssetBlindingFactor {
        self.inner.asset_bf.into()
    }

    /// Return the value of the output.
    pub fn value(&self) -> u64 {
        self.inner.value
    }

    /// Return the value blinding factor as a typed object.
    #[wasm_bindgen(js_name = valueBlindingFactor)]
    pub fn value_blinding_factor(&self) -> blinding_factor::ValueBlindingFactor {
        self.inner.value_bf.into()
    }

    /// Return true if the output is explicit (no blinding factors).
    #[wasm_bindgen(js_name = isExplicit)]
    pub fn is_explicit(&self) -> bool {
        self.inner.asset_bf == AssetBlindingFactor::zero()
            && self.inner.value_bf == ValueBlindingFactor::zero()
    }

    /// Get the asset commitment
    ///
    /// If the output is explicit, returns the empty string
    #[wasm_bindgen(js_name = assetCommitment)]
    pub fn asset_commitment(&self) -> String {
        if self.is_explicit() {
            "".to_string()
        } else {
            self.asset_generator()
                .to_string()
                .parse()
                .expect("from pedersen commitment")
        }
    }

    /// Get the value commitment
    ///
    /// If the output is explicit, returns the empty string
    #[wasm_bindgen(js_name = valueCommitment)]
    pub fn value_commitment(&self) -> String {
        if self.is_explicit() {
            "".to_string()
        } else {
            let value = self.inner.value;
            let vbf = self.inner.value_bf.into_inner();

            PedersenCommitment::new(&EC, value, vbf, self.asset_generator())
                .to_string()
                .parse()
                .expect("from pedersen commitment")
        }
    }
}

impl TxOutSecrets {
    fn asset_generator(&self) -> Generator {
        let asset = self.inner.asset.into_inner().to_byte_array();
        let abf = self.inner.asset_bf.into_inner();
        let asset_tag = Tag::from(asset);
        Generator::new_blinded(&EC, asset_tag, abf)
    }
}

#[cfg(all(test, target_arch = "wasm32", feature = "simplicity"))]
mod tests {
    use std::str::FromStr;

    use lwk_wollet::elements;
    use lwk_wollet::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use lwk_wollet::elements::hex::FromHex;
    use lwk_wollet::elements::AssetId;

    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn tx_out_secrets() {
        let zero_hex = "0000000000000000000000000000000000000000000000000000000000000000";
        let asset_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";

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
            txoutsecrets_explicit.asset_blinding_factor().to_string_js(),
            zero_hex
        );
        assert_eq!(
            txoutsecrets_explicit.value_blinding_factor().to_string_js(),
            zero_hex
        );
        assert_eq!(txoutsecrets_explicit.asset_commitment().to_string(), "");
        assert_eq!(txoutsecrets_explicit.value_commitment().to_string(), "");

        let abf_hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let vbf_hex = "0000570186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let vc_hex = "08ce46e7961ad9f5bf44156f80e073ea984dc11178557790dbc2eff9cdafecbe35";
        let ac_hex = "0ba1bed30ab07f7b215f812df37dde5b0338a48496b60e0584af102c4a7d766837";
        let blinded_secrets: crate::TxOutSecrets = elements::TxOutSecrets::new(
            AssetId::from_str(asset_hex).unwrap(),
            AssetBlindingFactor::from_hex(abf_hex).unwrap(),
            1000,
            ValueBlindingFactor::from_hex(vbf_hex).unwrap(),
        )
        .into();

        assert!(!blinded_secrets.is_explicit());
        assert_eq!(blinded_secrets.value(), 1000);
        assert_eq!(blinded_secrets.asset().to_string(), asset_hex);
        assert_eq!(
            blinded_secrets.asset_blinding_factor().to_string_js(),
            abf_hex
        );
        assert_eq!(
            blinded_secrets.value_blinding_factor().to_string_js(),
            vbf_hex
        );
        assert_eq!(blinded_secrets.asset_commitment().to_string(), ac_hex);
        assert_eq!(blinded_secrets.value_commitment().to_string(), vc_hex);

        let asset_id = crate::AssetId::from_string(asset_hex).unwrap();
        let explicit = crate::TxOutSecrets::from_explicit(&asset_id, 5000);
        assert!(explicit.is_explicit());
        assert_eq!(explicit.value(), 5000);
    }
}
