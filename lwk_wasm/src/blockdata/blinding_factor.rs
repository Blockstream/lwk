use crate::Error;

use std::str::FromStr;

use lwk_wollet::elements::confidential;

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
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<AssetBlindingFactor, Error> {
        let inner = confidential::AssetBlindingFactor::from_slice(bytes)?;
        Ok(AssetBlindingFactor { inner })
    }

    /// Returns a zero asset blinding factor.
    pub fn zero() -> AssetBlindingFactor {
        AssetBlindingFactor {
            inner: confidential::AssetBlindingFactor::zero(),
        }
    }

    /// Returns the bytes (32 bytes) in little-endian byte order.
    ///
    /// This is the internal representation used by secp256k1. The byte order is
    /// reversed compared to the hex string representation (which uses big-endian,
    /// following Bitcoin display conventions).
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.into_inner().as_ref().to_vec()
    }

    /// Returns string representation of the ABF
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.inner.to_string()
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
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<ValueBlindingFactor, Error> {
        let inner = confidential::ValueBlindingFactor::from_slice(bytes)?;
        Ok(ValueBlindingFactor { inner })
    }

    /// Returns a zero value blinding factor.
    pub fn zero() -> ValueBlindingFactor {
        ValueBlindingFactor {
            inner: confidential::ValueBlindingFactor::zero(),
        }
    }

    /// Returns the bytes (32 bytes) in little-endian byte order.
    ///
    /// This is the internal representation used by secp256k1. The byte order is
    /// reversed compared to the hex string representation (which uses big-endian,
    /// following Bitcoin display conventions).
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.into_inner().as_ref().to_vec()
    }

    /// Returns string representation of the VBF
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.inner.to_string()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    fn reverse_hex(hex: &str) -> String {
        hex.as_bytes()
            .chunks(2)
            .rev()
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect()
    }

    #[wasm_bindgen_test]
    fn test_asset_blinding_factor() {
        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let abf = AssetBlindingFactor::new(hex).unwrap();
        assert_eq!(abf.to_string_js(), hex);
        assert_eq!(abf.bytes().to_hex(), reverse_hex(hex));

        let bytes = abf.bytes();
        let abf2 = AssetBlindingFactor::from_bytes(&bytes).unwrap();
        assert_eq!(abf, abf2);

        let zero = AssetBlindingFactor::zero();
        assert_eq!(zero.bytes(), vec![0u8; 32]);

        assert!(AssetBlindingFactor::new("invalid").is_err());
        assert!(AssetBlindingFactor::from_bytes(&[0u8; 16]).is_err());
    }

    #[wasm_bindgen_test]
    fn test_value_blinding_factor() {
        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let vbf = ValueBlindingFactor::new(hex).unwrap();
        assert_eq!(vbf.to_string_js(), hex);
        assert_eq!(vbf.bytes().to_hex(), reverse_hex(hex));

        let bytes = vbf.bytes();
        let vbf2 = ValueBlindingFactor::from_bytes(&bytes).unwrap();
        assert_eq!(vbf, vbf2);

        let zero = ValueBlindingFactor::zero();
        assert_eq!(zero.bytes(), vec![0u8; 32]);

        assert!(ValueBlindingFactor::new("invalid").is_err());
        assert!(ValueBlindingFactor::from_bytes(&[0u8; 16]).is_err());
    }
}
