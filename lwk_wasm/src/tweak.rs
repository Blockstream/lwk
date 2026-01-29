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
    fn test_tweak() {
        let zero = Tweak::zero();
        assert_eq!(zero.to_bytes(), vec![0u8; 32]);

        let bytes = [1u8; 32];
        let tweak = Tweak::new(&bytes).unwrap();
        assert_eq!(tweak.to_bytes(), bytes);

        let hex = "0100000000000000000000000000000000000000000000000000000000000000";
        let tweak_hex = Tweak::from_hex(hex).unwrap();
        assert_eq!(tweak_hex.to_hex(), hex);
        let tweak_hex2 = Tweak::from_hex(&tweak_hex.to_hex()).unwrap();
        assert_eq!(tweak_hex, tweak_hex2);

        let tweak2 = Tweak::new(&tweak.to_bytes()).unwrap();
        assert_eq!(tweak, tweak2);

        assert_eq!(tweak_hex.to_string(), hex);
        assert_eq!(tweak_hex.to_string_js(), hex);

        assert!(Tweak::new(&[0; 31]).is_err());
        assert!(Tweak::new(&[0; 33]).is_err());
        assert!(Tweak::new(&[]).is_err());

        assert!(Tweak::from_hex("aabb").is_err());
        assert!(Tweak::from_hex("invalid").is_err());
    }
}
