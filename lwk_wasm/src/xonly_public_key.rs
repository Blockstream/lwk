use crate::Error;

use std::fmt::Display;
use std::str::FromStr;

use lwk_wollet::bitcoin;
use lwk_wollet::elements::hex::ToHex;

use wasm_bindgen::prelude::*;

/// An x-only public key, used for verification of Taproot signatures and serialized according to BIP-340.
///
/// See [`bitcoin::XOnlyPublicKey`] for more details.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct XOnlyPublicKey {
    inner: bitcoin::XOnlyPublicKey,
}

impl From<bitcoin::XOnlyPublicKey> for XOnlyPublicKey {
    fn from(inner: bitcoin::XOnlyPublicKey) -> Self {
        XOnlyPublicKey { inner }
    }
}

impl From<XOnlyPublicKey> for bitcoin::XOnlyPublicKey {
    fn from(value: XOnlyPublicKey) -> Self {
        value.inner
    }
}

impl From<&XOnlyPublicKey> for bitcoin::XOnlyPublicKey {
    fn from(value: &XOnlyPublicKey) -> Self {
        value.inner
    }
}

impl AsRef<bitcoin::XOnlyPublicKey> for XOnlyPublicKey {
    fn as_ref(&self) -> &bitcoin::XOnlyPublicKey {
        &self.inner
    }
}

impl Display for XOnlyPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl XOnlyPublicKey {
    /// Creates an `XOnlyPublicKey` from a hex string (64 hex characters = 32 bytes).
    #[wasm_bindgen(constructor)]
    pub fn new(hex: &str) -> Result<XOnlyPublicKey, Error> {
        let inner = bitcoin::XOnlyPublicKey::from_str(hex)?;
        Ok(XOnlyPublicKey { inner })
    }

    /// Creates an `XOnlyPublicKey` from raw bytes (32 bytes).
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<XOnlyPublicKey, Error> {
        let inner = bitcoin::XOnlyPublicKey::from_slice(bytes)?;
        Ok(XOnlyPublicKey { inner })
    }

    /// Serializes the x-only public key to bytes (32 bytes).
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.serialize().to_vec()
    }

    /// Returns the hex-encoded serialization (64 hex characters).
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.serialize().to_hex()
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
    use lwk_wollet::hashes::hex::FromHex;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_xonly_public_key() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(hex).unwrap();

        let key = XOnlyPublicKey::new(hex).unwrap();
        assert_eq!(key.to_hex(), hex);

        let key_from_bytes = XOnlyPublicKey::from_bytes(&bytes).unwrap();
        assert_eq!(key_from_bytes.to_bytes(), bytes);

        let key2 = XOnlyPublicKey::new(&key.to_hex()).unwrap();
        assert_eq!(key, key2);

        let key3 = XOnlyPublicKey::from_bytes(&key.to_bytes()).unwrap();
        assert_eq!(key, key3);

        assert_eq!(key.to_string(), hex);
        assert_eq!(key.to_string_js(), hex);

        assert!(XOnlyPublicKey::new("aabb").is_err());
        assert!(XOnlyPublicKey::new(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f8179800"
        )
        .is_err());
        assert!(XOnlyPublicKey::new(
            "xx79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f817"
        )
        .is_err());

        assert!(XOnlyPublicKey::from_bytes(&[0; 31]).is_err());
        assert!(XOnlyPublicKey::from_bytes(&[0; 33]).is_err());
        assert!(XOnlyPublicKey::from_bytes(&[]).is_err());
    }
}
