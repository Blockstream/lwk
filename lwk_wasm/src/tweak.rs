use crate::Error;

use std::fmt::Display;
use std::str::FromStr;

use lwk_wollet::elements::secp256k1_zkp;

use wasm_bindgen::prelude::*;

/// Represents a blinding factor/Tweak on secp256k1 curve.
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
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Tweak, Error> {
        let inner = secp256k1_zkp::Tweak::from_slice(bytes)?;
        Ok(Tweak { inner })
    }

    /// Create a Tweak from a string.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(s: &str) -> Result<Tweak, Error> {
        let inner = secp256k1_zkp::Tweak::from_str(s)?;
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
        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";

        let from_hex = Tweak::from_string(hex).unwrap();
        assert_eq!(from_hex.to_string(), hex);

        let bytes = from_hex.to_bytes();
        let from_bytes = Tweak::from_bytes(&bytes).unwrap();
        assert_eq!(from_bytes.to_bytes(), bytes);
        assert_eq!(from_bytes.to_string(), hex);
        assert_eq!(
            Tweak::from_string(&from_bytes.to_string()).unwrap(),
            from_bytes
        );

        assert!(Tweak::from_bytes(&[0u8; 31]).is_err());
        assert!(Tweak::from_bytes(&[0u8; 33]).is_err());

        let tweak = Tweak::zero();
        assert_eq!(tweak.to_bytes(), vec![0u8; 32]);
        assert_eq!(
            tweak.to_string(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
    }
}
