use crate::{Error, SecretKey, XOnlyPublicKey};

use std::fmt::Display;
use std::str::FromStr;

use lwk_wollet::elements::bitcoin::secp256k1;
use lwk_wollet::elements_miniscript::ToPublicKey;

use wasm_bindgen::prelude::*;

/// A Bitcoin ECDSA public key
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct PublicKey {
    inner: lwk_wollet::elements::bitcoin::PublicKey,
}

impl From<lwk_wollet::elements::bitcoin::PublicKey> for PublicKey {
    fn from(inner: lwk_wollet::elements::bitcoin::PublicKey) -> Self {
        PublicKey { inner }
    }
}

impl From<PublicKey> for lwk_wollet::elements::bitcoin::PublicKey {
    fn from(value: PublicKey) -> Self {
        value.inner
    }
}

impl From<&PublicKey> for lwk_wollet::elements::bitcoin::PublicKey {
    fn from(value: &PublicKey) -> Self {
        value.inner
    }
}

impl FromStr for PublicKey {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = lwk_wollet::elements::bitcoin::PublicKey::from_str(s)?;
        Ok(Self { inner })
    }
}

impl Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl PublicKey {
    /// Creates a `PublicKey` from a string.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(s: &str) -> Result<PublicKey, Error> {
        Self::from_str(s)
    }

    /// Creates a `PublicKey` from a byte array (33 or 65 bytes)
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<PublicKey, Error> {
        let inner = lwk_wollet::elements::bitcoin::PublicKey::from_slice(bytes)?;
        Ok(PublicKey { inner })
    }

    /// Derives a compressed `PublicKey` from a `SecretKey`
    #[wasm_bindgen(js_name = fromSecretKey)]
    pub fn from_secret_key(sk: &SecretKey) -> PublicKey {
        let secp = secp256k1::Secp256k1::new();
        let secret: secp256k1::SecretKey = sk.into();
        let inner_pk = secp256k1::PublicKey::from_secret_key(&secp, &secret);
        PublicKey {
            inner: lwk_wollet::elements::bitcoin::PublicKey::new(inner_pk),
        }
    }

    /// Serializes the public key to bytes
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes()
    }

    /// Returns whether this public key is compressed (33 bytes) or uncompressed (65 bytes)
    #[wasm_bindgen(js_name = isCompressed)]
    pub fn is_compressed(&self) -> bool {
        self.inner.compressed
    }

    /// Converts to an x-only public key
    #[wasm_bindgen(js_name = toXOnlyPublicKey)]
    pub fn to_x_only_public_key(&self) -> XOnlyPublicKey {
        self.inner.to_x_only_pubkey().into()
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

    use lwk_wollet::hashes::hex::FromHex;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_public_key() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(hex).unwrap();

        let from_hex = PublicKey::from_string(hex).unwrap();
        assert_eq!(from_hex.to_string(), hex);
        assert_eq!(from_hex.to_bytes(), bytes);
        assert!(from_hex.is_compressed());

        let from_bytes = PublicKey::from_bytes(&bytes).unwrap();
        assert_eq!(from_bytes.to_bytes(), bytes);
        assert_eq!(from_hex, from_bytes);

        let mut secret_key_bytes = [0x11; 32];
        secret_key_bytes[31] = 0x22;
        let sk = SecretKey::from_bytes(&secret_key_bytes).unwrap();
        let from_secret_key = PublicKey::from_secret_key(&sk);
        assert_eq!(from_secret_key.to_bytes().len(), 33);
        assert!(from_secret_key.is_compressed());

        let xonly = from_hex.to_x_only_public_key();
        assert_eq!(
            xonly.to_string(),
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );

        assert!(PublicKey::from_bytes(&[0; 32]).is_err());
        assert!(PublicKey::from_bytes(&[0; 33]).is_err());

        let hex = "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";
        let pk = PublicKey::from_string(hex).unwrap();
        assert!(!pk.is_compressed());
    }
}
