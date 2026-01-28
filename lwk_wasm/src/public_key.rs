use crate::{Error, SecretKey};

use std::fmt::Display;

use lwk_wollet::elements::bitcoin::secp256k1;
use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::elements_miniscript::ToPublicKey;
use lwk_wollet::hashes::hex::FromHex;

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

impl Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl PublicKey {
    /// Creates a `PublicKey` from a hex string
    #[wasm_bindgen(constructor)]
    pub fn new(hex: &str) -> Result<PublicKey, Error> {
        let bytes = Vec::<u8>::from_hex(hex)?;
        Self::from_bytes(&bytes)
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

    /// Returns the hex-encoded serialization
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.to_bytes().to_hex()
    }

    /// Returns whether this public key is compressed (33 bytes) or uncompressed (65 bytes)
    #[wasm_bindgen(js_name = isCompressed)]
    pub fn is_compressed(&self) -> bool {
        self.inner.compressed
    }

    /// Converts to an x-only public key hex string (32 bytes hex-encoded)
    #[wasm_bindgen(js_name = toXOnlyHex)]
    pub fn to_x_only_hex(&self) -> String {
        self.inner.to_x_only_pubkey().to_string()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use crate::SecretKey;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_public_key_from_hex() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pk = PublicKey::new(hex).unwrap();
        assert_eq!(pk.to_hex(), hex);
        assert!(pk.is_compressed());
    }

    #[wasm_bindgen_test]
    fn test_public_key_from_bytes() {
        let bytes = Vec::<u8>::from_hex(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .unwrap();
        let pk = PublicKey::from_bytes(&bytes).unwrap();
        assert_eq!(pk.to_bytes(), bytes);
    }

    #[wasm_bindgen_test]
    fn test_public_key_from_secret_key() {
        let sk = SecretKey::new(&[1u8; 32]).unwrap();
        let pk = PublicKey::from_secret_key(&sk);
        assert_eq!(pk.to_bytes().len(), 33);
        assert!(pk.is_compressed());
    }

    #[wasm_bindgen_test]
    fn test_public_key_to_x_only() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pk = PublicKey::new(hex).unwrap();
        let xonly = pk.to_x_only_hex();
        assert_eq!(xonly.len(), 64);
        assert_eq!(
            xonly,
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );
    }

    #[wasm_bindgen_test]
    fn test_public_key_uncompressed() {
        let hex = "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";
        let pk = PublicKey::new(hex).unwrap();
        assert!(!pk.is_compressed());
        assert_eq!(pk.to_bytes().len(), 65);
    }

    #[wasm_bindgen_test]
    fn test_public_key_invalid() {
        assert!(PublicKey::from_bytes(&[0; 32]).is_err());
        assert!(PublicKey::from_bytes(&[0; 34]).is_err());
        assert!(PublicKey::from_bytes(&[0; 33]).is_err());
    }

    #[wasm_bindgen_test]
    fn test_public_key_roundtrip() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pk = PublicKey::new(hex).unwrap();
        let pk2 = PublicKey::from_bytes(&pk.to_bytes()).unwrap();
        assert_eq!(pk, pk2);
    }
}
