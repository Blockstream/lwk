use crate::Error;

use std::str::FromStr;

use lwk_wollet::elements::confidential;
use lwk_wollet::elements::hex::ToHex;

use wasm_bindgen::prelude::*;

/// A blinding factor for asset commitments.
///
/// See [`confidential::AssetBlindingFactor`] for more details.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct AssetBlindingFactor {
    inner: confidential::AssetBlindingFactor,
}

impl From<confidential::AssetBlindingFactor> for AssetBlindingFactor {
    fn from(inner: confidential::AssetBlindingFactor) -> Self {
        AssetBlindingFactor { inner }
    }
}

impl From<AssetBlindingFactor> for confidential::AssetBlindingFactor {
    fn from(value: AssetBlindingFactor) -> Self {
        value.inner
    }
}

impl From<&AssetBlindingFactor> for confidential::AssetBlindingFactor {
    fn from(value: &AssetBlindingFactor) -> Self {
        value.inner
    }
}

impl AsRef<confidential::AssetBlindingFactor> for AssetBlindingFactor {
    fn as_ref(&self) -> &confidential::AssetBlindingFactor {
        &self.inner
    }
}

#[wasm_bindgen]
impl AssetBlindingFactor {
    /// Creates an `AssetBlindingFactor` from a hex string.
    #[wasm_bindgen(constructor)]
    pub fn new(hex: &str) -> Result<AssetBlindingFactor, Error> {
        let inner = confidential::AssetBlindingFactor::from_str(hex)?;
        Ok(AssetBlindingFactor { inner })
    }

    /// Creates an `AssetBlindingFactor` from a byte slice.
    #[wasm_bindgen(js_name = fromSlice)]
    pub fn from_slice(bytes: &[u8]) -> Result<AssetBlindingFactor, Error> {
        let inner = confidential::AssetBlindingFactor::from_slice(bytes)?;
        Ok(AssetBlindingFactor { inner })
    }

    /// Returns a zero asset blinding factor.
    pub fn zero() -> AssetBlindingFactor {
        AssetBlindingFactor {
            inner: confidential::AssetBlindingFactor::zero(),
        }
    }

    /// Returns the bytes (32 bytes).
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.into_inner().as_ref().to_vec()
    }

    /// Returns the hex-encoded representation.
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.into_inner().as_ref().to_hex()
    }
}

/// A blinding factor for value commitments.
///
/// See [`confidential::ValueBlindingFactor`] for more details.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct ValueBlindingFactor {
    inner: confidential::ValueBlindingFactor,
}

impl From<confidential::ValueBlindingFactor> for ValueBlindingFactor {
    fn from(inner: confidential::ValueBlindingFactor) -> Self {
        ValueBlindingFactor { inner }
    }
}

impl From<ValueBlindingFactor> for confidential::ValueBlindingFactor {
    fn from(value: ValueBlindingFactor) -> Self {
        value.inner
    }
}

impl From<&ValueBlindingFactor> for confidential::ValueBlindingFactor {
    fn from(value: &ValueBlindingFactor) -> Self {
        value.inner
    }
}

impl AsRef<confidential::ValueBlindingFactor> for ValueBlindingFactor {
    fn as_ref(&self) -> &confidential::ValueBlindingFactor {
        &self.inner
    }
}

#[wasm_bindgen]
impl ValueBlindingFactor {
    /// Creates a `ValueBlindingFactor` from a hex string.
    #[wasm_bindgen(constructor)]
    pub fn new(hex: &str) -> Result<ValueBlindingFactor, Error> {
        let inner = confidential::ValueBlindingFactor::from_str(hex)?;
        Ok(ValueBlindingFactor { inner })
    }

    /// Creates a `ValueBlindingFactor` from a byte slice.
    #[wasm_bindgen(js_name = fromSlice)]
    pub fn from_slice(bytes: &[u8]) -> Result<ValueBlindingFactor, Error> {
        let inner = confidential::ValueBlindingFactor::from_slice(bytes)?;
        Ok(ValueBlindingFactor { inner })
    }

    /// Returns a zero value blinding factor.
    pub fn zero() -> ValueBlindingFactor {
        ValueBlindingFactor {
            inner: confidential::ValueBlindingFactor::zero(),
        }
    }

    /// Returns the bytes (32 bytes).
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.into_inner().as_ref().to_vec()
    }

    /// Returns the hex-encoded representation.
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.into_inner().as_ref().to_hex()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_asset_blinding_factor() {
        let hex = "0101010101010101010101010101010101010101010101010101010101010101";
        let abf = AssetBlindingFactor::new(hex).unwrap();
        assert_eq!(abf.to_hex(), hex);

        let bytes = abf.to_bytes();
        let abf2 = AssetBlindingFactor::from_slice(&bytes).unwrap();
        assert_eq!(abf, abf2);

        let zero = AssetBlindingFactor::zero();
        assert_eq!(zero.to_bytes(), vec![0u8; 32]);

        assert!(AssetBlindingFactor::new("invalid").is_err());
        assert!(AssetBlindingFactor::from_slice(&[0u8; 16]).is_err());
    }

    #[wasm_bindgen_test]
    fn test_value_blinding_factor() {
        let hex = "0202020202020202020202020202020202020202020202020202020202020202";
        let vbf = ValueBlindingFactor::new(hex).unwrap();
        assert_eq!(vbf.to_hex(), hex);

        let bytes = vbf.to_bytes();
        let vbf2 = ValueBlindingFactor::from_slice(&bytes).unwrap();
        assert_eq!(vbf, vbf2);

        let zero = ValueBlindingFactor::zero();
        assert_eq!(zero.to_bytes(), vec![0u8; 32]);

        assert!(ValueBlindingFactor::new("invalid").is_err());
        assert!(ValueBlindingFactor::from_slice(&[0u8; 16]).is_err());
    }
}
