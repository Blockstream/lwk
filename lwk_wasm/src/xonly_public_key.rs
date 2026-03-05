use crate::Error;

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use lwk_wollet::elements::bitcoin;

use wasm_bindgen::prelude::*;

/// An x-only public key, used for verification of Taproot signatures and serialized according to BIP-340.
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
    /// Creates an `XOnlyPublicKey` from a string.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(s: &str) -> Result<XOnlyPublicKey, Error> {
        let inner = bitcoin::XOnlyPublicKey::from_str(s)?;
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

    /// Returns the string representation.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

impl XOnlyPublicKey {
    /// Create from a keypair, returning the x-only public key
    pub fn from_keypair(keypair: &bitcoin::secp256k1::Keypair) -> Arc<Self> {
        let (xonly, _parity) = keypair.x_only_public_key();
        Arc::new(Self::from(xonly))
    }
}

#[cfg(feature = "simplicity")]
impl XOnlyPublicKey {
    /// Convert to simplicityhl XOnlyPublicKey
    /// TODO: delete when the version of elements is stabilized
    pub(crate) fn to_simplicityhl(
        self,
    ) -> Result<lwk_simplicity::simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey, Error> {
        Ok(
            lwk_simplicity::simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey::from_slice(
                &self.inner.serialize(),
            )?,
        )
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    use lwk_simplicity::simplicityhl::elements::schnorr::Keypair;
    use lwk_simplicity::simplicityhl::elements::secp256k1_zkp::SecretKey;
    use lwk_wollet::EC;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_xonly_public_key() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

        let from_hex = XOnlyPublicKey::from_string(hex).unwrap();
        assert_eq!(from_hex.to_string(), hex);

        let from_str = XOnlyPublicKey::from_string(hex).unwrap();
        assert_eq!(from_str, from_hex);

        let bytes = from_hex.to_bytes();
        let from_bytes = XOnlyPublicKey::from_bytes(&bytes).unwrap();
        assert_eq!(from_bytes.to_bytes(), bytes);
        assert_eq!(from_hex, from_bytes);

        let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let keypair = Keypair::from_secret_key(&EC, &secret_key);

        let xonly = XOnlyPublicKey::from_keypair(&keypair);
        let (expected, _parity) = keypair.x_only_public_key();

        assert_eq!(xonly.to_bytes(), expected.serialize());

        assert!(XOnlyPublicKey::from_bytes(&[0; 31]).is_err());
        assert!(XOnlyPublicKey::from_bytes(&[0; 33]).is_err());

        let too_long_hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f8179800";
        let invalid_hex = "xx79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f817";

        assert!(XOnlyPublicKey::from_string(too_long_hex).is_err());
        assert!(XOnlyPublicKey::from_string(invalid_hex).is_err());
    }
}
