use std::{fmt::Display, str::FromStr};

use crate::{LwkError, UniffiCustomTypeConverter};

/// An x-only public key, used for verification of Taproot signatures
/// and serialized according to BIP-340.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct XOnlyPublicKey {
    inner: elements::bitcoin::XOnlyPublicKey,
}

impl XOnlyPublicKey {
    /// Create from a keypair, returning the x-only public key.
    #[must_use]
    pub fn from_keypair(keypair: &elements::bitcoin::secp256k1::Keypair) -> Self {
        let (xonly, _parity) = keypair.x_only_public_key();
        Self::from(xonly)
    }

    /// Create from raw bytes (32 bytes).
    pub fn from_slice(bytes: &[u8]) -> Result<Self, LwkError> {
        let inner = elements::bitcoin::XOnlyPublicKey::from_slice(bytes).map_err(|e| {
            LwkError::Generic {
                msg: format!("Invalid x-only public key: {e}"),
            }
        })?;

        Ok(Self { inner })
    }

    /// Deserialize from 32 bytes. Alias for `from_slice`.
    pub fn deserialize(bytes: &[u8; 32]) -> Result<Self, LwkError> {
        Self::from_slice(bytes)
    }

    /// Serialize to 32 bytes.
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

uniffi::custom_type!(XOnlyPublicKey, String);
impl UniffiCustomTypeConverter for XOnlyPublicKey {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        Ok(Self::from_str(&val)?)
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use elements::bitcoin::secp256k1::{Keypair, Secp256k1, SecretKey};

    use super::XOnlyPublicKey;
    use crate::UniffiCustomTypeConverter;

    #[test]
    fn xonly_public_key_roundtrip() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let key = XOnlyPublicKey::from_str(hex).unwrap();

        assert_eq!(
            <XOnlyPublicKey as UniffiCustomTypeConverter>::into_custom(
                UniffiCustomTypeConverter::from_custom(key)
            )
            .unwrap(),
            key
        );
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
    fn xonly_public_key_serialize_deserialize_roundtrip() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let key = XOnlyPublicKey::from_str(hex).unwrap();

        let serialized = key.serialize();
        let deserialized = XOnlyPublicKey::deserialize(&serialized).unwrap();

        assert_eq!(key, deserialized);
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
    fn xonly_public_key_from_str_invalid() {
        // Too short
        assert!(XOnlyPublicKey::from_str("aabb").is_err());
        // Too long
        assert!(XOnlyPublicKey::from_str(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f8179800"
        )
        .is_err());
        // Invalid hex
        assert!(XOnlyPublicKey::from_str(
            "xx79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f817"
        )
        .is_err());
    }

    #[test]
    fn xonly_public_key_display() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let key = XOnlyPublicKey::from_str(hex).unwrap();

        assert_eq!(key.to_string(), hex);
    }
}
