use super::SecretKey;

use crate::{LwkError, XOnlyPublicKey};

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::bitcoin::secp256k1;
use elements::hashes::hex::FromHex;
use elements::hex::ToHex;

use lwk_wollet::elements_miniscript::ToPublicKey;

/// A Bitcoin ECDSA public key.
///
/// See [`elements::bitcoin::PublicKey`] for more details.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
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
    /// See [`elements::bitcoin::PublicKey::from_slice`].
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::bitcoin::PublicKey::from_slice(bytes)?;
        Ok(Arc::new(PublicKey { inner }))
    }

    /// Creates a `PublicKey` from a hex string.
    #[uniffi::constructor]
    pub fn from_hex(hex: &str) -> Result<Arc<Self>, LwkError> {
        let bytes = Vec::<u8>::from_hex(hex)?;
        Self::from_bytes(&bytes)
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

    /// See [`elements::bitcoin::PublicKey::to_bytes`].
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_bytes()
    }

    /// Returns the hex-encoded serialization.
    pub fn to_hex(&self) -> String {
        self.inner.to_bytes().to_hex()
    }

    /// See [`elements::bitcoin::PublicKey::compressed`].
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
        lwk_simplicity::simplicityhl::elements::bitcoin::PublicKey::from_slice(&self.to_bytes())
            .map_err(|e| LwkError::Generic {
                msg: format!("Invalid public key: {e}"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{PublicKey, SecretKey};
    use elements::hashes::hex::FromHex;

    #[test]
    fn test_public_key_from_bytes() {
        let bytes = Vec::<u8>::from_hex(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .unwrap();
        let pk = PublicKey::from_bytes(&bytes).unwrap();
        assert_eq!(pk.to_bytes(), bytes);
        assert!(pk.is_compressed());
    }

    #[test]
    fn test_public_key_from_hex() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pk = PublicKey::from_hex(hex).unwrap();
        assert_eq!(pk.to_hex(), hex);
    }

    #[test]
    fn test_public_key_from_secret_key() {
        let sk = SecretKey::from_bytes(&[1u8; 32]).unwrap();
        let pk = PublicKey::from_secret_key(&sk);
        assert_eq!(pk.to_bytes().len(), 33);
        assert!(pk.is_compressed());
    }

    #[test]
    fn test_public_key_invalid() {
        assert!(PublicKey::from_bytes(&[0; 32]).is_err());
        assert!(PublicKey::from_bytes(&[0; 34]).is_err());
        assert!(PublicKey::from_bytes(&[0; 33]).is_err());
    }

    #[test]
    fn test_public_key_roundtrip() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pk = PublicKey::from_hex(hex).unwrap();
        let pk2 = PublicKey::from_bytes(&pk.to_bytes()).unwrap();
        assert_eq!(*pk, *pk2);
    }

    #[test]
    fn test_public_key_to_x_only() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pk = PublicKey::from_hex(hex).unwrap();
        let xonly = pk.to_x_only_public_key();
        assert_eq!(xonly.to_string().len(), 64);
        assert_eq!(
            xonly.to_string(),
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );
    }

    #[test]
    fn test_public_key_display() {
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pk = PublicKey::from_hex(hex).unwrap();
        assert_eq!(pk.to_string(), hex);
    }

    #[test]
    fn test_public_key_as_ref() {
        use elements::bitcoin::PublicKey as BitcoinPublicKey;
        let hex = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let pk = PublicKey::from_hex(hex).unwrap();
        let inner: &BitcoinPublicKey = (*pk).as_ref();
        assert!(inner.compressed);
    }

    #[test]
    fn test_public_key_uncompressed() {
        let hex = "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";
        let pk = PublicKey::from_hex(hex).unwrap();
        assert!(!pk.is_compressed());
        assert_eq!(pk.to_bytes().len(), 65);
    }
}
