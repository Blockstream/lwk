use super::SecretKey;

use crate::{LwkError, XOnlyPublicKey};

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::bitcoin::secp256k1;

use lwk_wollet::elements_miniscript::ToPublicKey;

/// A Bitcoin ECDSA public key.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
#[uniffi::export(Display)]
pub struct PublicKey {
    inner: elements::bitcoin::PublicKey,
}

impl From<elements::bitcoin::PublicKey> for PublicKey {
    fn from(inner: elements::bitcoin::PublicKey) -> Self {
        PublicKey { inner }
    }
}

impl From<PublicKey> for elements::bitcoin::PublicKey {
    fn from(value: PublicKey) -> Self {
        value.inner
    }
}

impl From<&PublicKey> for elements::bitcoin::PublicKey {
    fn from(value: &PublicKey) -> Self {
        value.inner
    }
}

impl AsRef<elements::bitcoin::PublicKey> for PublicKey {
    fn as_ref(&self) -> &elements::bitcoin::PublicKey {
        &self.inner
    }
}

impl FromStr for PublicKey {
    type Err = LwkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = elements::bitcoin::PublicKey::from_str(s)?;
        Ok(Self { inner })
    }
}

impl Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl PublicKey {
    /// Deserialize a public key from bytes
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::bitcoin::PublicKey::from_slice(bytes)?;
        Ok(Arc::new(PublicKey { inner }))
    }

    /// Creates a `PublicKey` from a hex string.
    #[uniffi::constructor]
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(s)?))
    }

    /// Derives a compressed `PublicKey` from a `SecretKey`.
    #[uniffi::constructor]
    pub fn from_secret_key(secret_key: &SecretKey) -> Arc<Self> {
        let secp = secp256k1::Secp256k1::new();
        let sk: secp256k1::SecretKey = secret_key.into();
        let inner_pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
        Arc::new(PublicKey {
            inner: elements::bitcoin::PublicKey::new(inner_pk),
        })
    }

    /// Serialize the public key to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes()
    }

    /// Whether this public key should be serialized as compressed
    pub fn is_compressed(&self) -> bool {
        self.inner.compressed
    }

    /// Converts to an x-only public key hex string.
    pub fn to_x_only_public_key(&self) -> XOnlyPublicKey {
        self.inner.to_x_only_pubkey().into()
    }
}

#[cfg(feature = "simplicity")]
impl PublicKey {
    /// Convert to simplicityhl's PublicKey type.
    ///
    /// TODO: delete when the version of elements is stabilized
    pub fn to_simplicityhl(
        &self,
    ) -> Result<lwk_simplicity::simplicityhl::elements::bitcoin::PublicKey, LwkError> {
        Ok(
            lwk_simplicity::simplicityhl::elements::bitcoin::PublicKey::from_slice(
                &self.to_bytes(),
            )?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use elements::hashes::hex::FromHex;

    #[test]
    fn test_public_key_constructors_roundtrip_and_views() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let bytes = Vec::<u8>::from_hex(hex).unwrap();

        let from_hex = PublicKey::from_string(hex).unwrap();
        assert_eq!(from_hex.to_string(), hex);
        assert_eq!(from_hex.to_bytes(), bytes);
        assert!(from_hex.is_compressed());

        let from_bytes = PublicKey::from_bytes(&bytes).unwrap();
        assert_eq!(from_bytes.to_bytes(), bytes);
        assert_eq!(*from_hex, *from_bytes);

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
