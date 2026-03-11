use crate::LwkError;

use std::{fmt::Display, str::FromStr, sync::Arc};

/// An x-only public key, used for verification of Taproot signatures
/// and serialized according to BIP-340.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
#[uniffi::export(Display)]
pub struct XOnlyPublicKey {
    inner: elements::bitcoin::XOnlyPublicKey,
}

#[uniffi::export]
impl XOnlyPublicKey {
    /// Create from a hex string.
    #[uniffi::constructor]
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(s)?))
    }

    /// Create from raw bytes (32 bytes).
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::bitcoin::XOnlyPublicKey::from_slice(bytes)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Serialize to 32 bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.serialize().to_vec()
    }
}

impl XOnlyPublicKey {
    /// Create from a keypair, returning the x-only public key
    pub fn from_keypair(keypair: &elements::bitcoin::secp256k1::Keypair) -> Arc<Self> {
        let (xonly, _parity) = keypair.x_only_public_key();
        Arc::new(Self::from(xonly))
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
    ) -> Result<lwk_simplicity::simplicityhl::simplicity::bitcoin::XOnlyPublicKey, LwkError> {
        Ok(
            lwk_simplicity::simplicityhl::simplicity::bitcoin::XOnlyPublicKey::from_slice(
                &self.to_bytes(),
            )?,
        )
    }
}

impl FromStr for XOnlyPublicKey {
    type Err = LwkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = elements::bitcoin::XOnlyPublicKey::from_str(s)?;

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
    use super::*;

    use lwk_wollet::secp256k1::{Keypair, SecretKey};
    use lwk_wollet::EC;

    #[test]
    fn xonly_public_key_constructors_roundtrip() {
        let hex = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

        let from_hex = XOnlyPublicKey::from_string(hex).unwrap();
        assert_eq!(from_hex.to_string(), hex);

        let from_str = XOnlyPublicKey::from_str(hex).unwrap();
        assert_eq!(from_str, *from_hex);

        let bytes = from_hex.to_bytes();
        let from_bytes = XOnlyPublicKey::from_bytes(&bytes).unwrap();
        assert_eq!(from_bytes.to_bytes(), bytes);
        assert_eq!(*from_hex, *from_bytes);

        let mut secret_key_bytes = [0x11; 32];
        secret_key_bytes[31] = 0x22;
        let secret_key = SecretKey::from_slice(&secret_key_bytes).unwrap();
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
