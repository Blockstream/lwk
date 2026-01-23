use crate::LwkError;

use std::{fmt::Display, str::FromStr, sync::Arc};

use elements::hex::ToHex;

/// An x-only public key, used for verification of Taproot signatures
/// and serialized according to BIP-340.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
pub struct XOnlyPublicKey {
    inner: elements::bitcoin::XOnlyPublicKey,
}

#[uniffi::export]
impl XOnlyPublicKey {
    /// Create from a hex string (64 hex characters = 32 bytes).
    #[uniffi::constructor]
    pub fn new(hex: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(hex)?))
    }

    /// Create from raw bytes (32 bytes).
    #[uniffi::constructor]
    pub fn from_slice(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::bitcoin::XOnlyPublicKey::from_slice(bytes).map_err(|e| {
            LwkError::Generic {
                msg: format!("Invalid x-only public key: {e}"),
            }
        })?;
        Ok(Arc::new(Self { inner }))
    }

    /// Serialize to 32 bytes.
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.serialize().to_vec()
    }

    /// Get hex representation of the x-only public key
    pub fn to_hex(&self) -> String {
        self.inner.to_hex()
    }
}

impl XOnlyPublicKey {
    /// Create from a keypair, returning the x-only public key
    #[must_use]
    pub fn from_keypair(keypair: &elements::bitcoin::secp256k1::Keypair) -> Arc<Self> {
        let (xonly, _parity) = keypair.x_only_public_key();
        Arc::new(Self::from(xonly))
    }

    /// Serialize to 32 bytes
    #[must_use]
    pub fn serialize(&self) -> [u8; 32] {
        self.inner.serialize()
    }
}

impl AsRef<elements::bitcoin::XOnlyPublicKey> for XOnlyPublicKey {
    fn as_ref(&self) -> &elements::bitcoin::XOnlyPublicKey {
        &self.inner
    }
}

impl From<elements::bitcoin::XOnlyPublicKey> for XOnlyPublicKey {
    fn from(inner: elements::bitcoin::XOnlyPublicKey) -> Self {
        Self { inner }
    }
}

impl From<XOnlyPublicKey> for elements::bitcoin::XOnlyPublicKey {
    fn from(key: XOnlyPublicKey) -> Self {
        key.inner
    }
}

#[cfg(feature = "simplicity")]
impl XOnlyPublicKey {
    /// Convert to simplicityhl's XOnlyPublicKey type for convinience
    /// TODO: delete when the version of elements is stabilized
    pub fn to_simplicityhl(
        &self,
    ) -> Result<lwk_simplicity_options::simplicityhl::simplicity::bitcoin::XOnlyPublicKey, LwkError>
    {
        lwk_simplicity_options::simplicityhl::simplicity::bitcoin::XOnlyPublicKey::from_slice(
            &self.serialize(),
        )
        .map_err(|e| LwkError::Generic {
            msg: format!("Invalid x-only public key: {e}"),
        })
    }
}

impl FromStr for XOnlyPublicKey {
    type Err = LwkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner =
            elements::bitcoin::XOnlyPublicKey::from_str(s).map_err(|e| LwkError::Generic {
                msg: format!("Invalid x-only public key: {e}"),
            })?;

        Ok(Self { inner })
    }
}

impl Display for XOnlyPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use elements::bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};

    use super::XOnlyPublicKey;

    #[test]
    fn xonly_public_key_roundtrip() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let key = XOnlyPublicKey::new(hex).unwrap();

        let as_string = key.to_string();
        let key2 = XOnlyPublicKey::new(&as_string).unwrap();
        assert_eq!(*key, *key2);
    }

    #[test]
    fn xonly_public_key_from_keypair() {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[1u8; 32]).unwrap();
        let keypair = Keypair::from_secret_key(&secp, &secret_key);

        let xonly = XOnlyPublicKey::from_keypair(&keypair);
        let (expected, _parity) = keypair.x_only_public_key();

        assert_eq!(xonly.serialize(), expected.serialize());
    }

    #[test]
    fn xonly_public_key_bytes_roundtrip() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let key = XOnlyPublicKey::from_str(hex).unwrap();

        let serialized = key.bytes();
        let deserialized = XOnlyPublicKey::from_slice(&serialized).unwrap();

        assert_eq!(key, *deserialized);
    }

    #[test]
    fn xonly_public_key_from_slice_bad_size() {
        // Too short
        assert!(XOnlyPublicKey::from_slice(&[0; 31]).is_err());
        // Too long
        assert!(XOnlyPublicKey::from_slice(&[0; 33]).is_err());
        // Empty
        assert!(XOnlyPublicKey::from_slice(&[]).is_err());
    }

    #[test]
    fn xonly_public_key_new_invalid() {
        // Too short
        assert!(XOnlyPublicKey::new("aabb").is_err());
        // Too long
        assert!(XOnlyPublicKey::new(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f8179800"
        )
        .is_err());
        // Invalid hex
        assert!(XOnlyPublicKey::new(
            "xx79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f817"
        )
        .is_err());
    }

    #[test]
    fn xonly_public_key_display() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let key = XOnlyPublicKey::new(hex).unwrap();

        assert_eq!(key.to_string(), hex);
    }
}
