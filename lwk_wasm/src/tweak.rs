use crate::Error;

use std::fmt::Display;
use std::str::FromStr;

use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::elements::secp256k1_zkp;

use wasm_bindgen::prelude::*;

/// Represents a blinding factor/Tweak on secp256k1 curve.
///
/// See [`secp256k1_zkp::Tweak`] for more details.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct Tweak {
    inner: secp256k1_zkp::Tweak,
}

impl From<secp256k1_zkp::Tweak> for Tweak {
    fn from(inner: secp256k1_zkp::Tweak) -> Self {
        Tweak { inner }
    }
}

impl From<Tweak> for secp256k1_zkp::Tweak {
    fn from(value: Tweak) -> Self {
        value.inner
    }
}

impl From<&Tweak> for secp256k1_zkp::Tweak {
    fn from(value: &Tweak) -> Self {
        value.inner
    }
}

impl AsRef<secp256k1_zkp::Tweak> for Tweak {
    fn as_ref(&self) -> &secp256k1_zkp::Tweak {
        &self.inner
    }
}

impl Display for Tweak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Tweak {
    /// Create a Tweak from a 32-byte slice.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<Tweak, Error> {
        let inner = secp256k1_zkp::Tweak::from_slice(bytes)?;
        Ok(Tweak { inner })
    }

    /// Create a Tweak from a hex string.
    #[wasm_bindgen(js_name = fromHex)]
    pub fn from_hex(hex: &str) -> Result<Tweak, Error> {
        let inner = secp256k1_zkp::Tweak::from_str(hex)?;
        Ok(Tweak { inner })
    }

    /// Create the zero tweak.
    pub fn zero() -> Tweak {
        Tweak {
            inner: secp256k1_zkp::ZERO_TWEAK,
        }
    }

    /// Return the bytes of the tweak (32 bytes).
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.as_ref().to_vec()
    }

    /// Return the hex representation of the tweak.
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.as_ref().to_hex()
    }

    /// Returns the string representation.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_tweak_zero() {
        let tweak = Tweak::zero();
        assert_eq!(tweak.to_bytes(), vec![0u8; 32]);
    }

    #[wasm_bindgen_test]
    fn test_tweak_from_bytes() {
        let bytes = [1u8; 32];
        let tweak = Tweak::new(&bytes).unwrap();
        assert_eq!(tweak.to_bytes(), bytes);
    }

    #[wasm_bindgen_test]
    fn test_tweak_from_hex() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let tweak = Tweak::from_hex(hex).unwrap();
        assert_eq!(tweak.to_hex(), hex);
    }

    #[wasm_bindgen_test]
    fn test_tweak_roundtrip_bytes() {
        let bytes = [2u8; 32];
        let tweak = Tweak::new(&bytes).unwrap();
        let tweak2 = Tweak::new(&tweak.to_bytes()).unwrap();
        assert_eq!(tweak, tweak2);
    }

    #[wasm_bindgen_test]
    fn test_tweak_roundtrip_hex() {
        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let tweak = Tweak::from_hex(hex).unwrap();
        let tweak2 = Tweak::from_hex(&tweak.to_hex()).unwrap();
        assert_eq!(tweak, tweak2);
    }

    #[wasm_bindgen_test]
    fn test_tweak_display() {
        let tweak = Tweak::zero();
        assert_eq!(
            tweak.to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(tweak.to_string_js(), tweak.to_string());
    }

    #[wasm_bindgen_test]
    fn test_tweak_invalid_size() {
        assert!(Tweak::new(&[0; 31]).is_err());
        assert!(Tweak::new(&[0; 33]).is_err());
        assert!(Tweak::new(&[]).is_err());
    }

    #[wasm_bindgen_test]
    fn test_tweak_invalid_hex() {
        assert!(Tweak::from_hex("aabb").is_err());
        assert!(Tweak::from_hex("invalid").is_err());
    }
}
