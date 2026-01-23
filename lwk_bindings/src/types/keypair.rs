use super::{PublicKey, SecretKey, XOnlyPublicKey};

use crate::LwkError;

use std::sync::Arc;

use elements::bitcoin::secp256k1::{self, Secp256k1};

/// A secp256k1 keypair.
///
/// See [`secp256k1::Keypair`] for more details.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone)]
pub struct Keypair {
    inner: secp256k1::Keypair,
}

impl From<secp256k1::Keypair> for Keypair {
    fn from(inner: secp256k1::Keypair) -> Self {
        Keypair { inner }
    }
}

impl From<Keypair> for secp256k1::Keypair {
    fn from(value: Keypair) -> Self {
        value.inner
    }
}

impl From<&Keypair> for secp256k1::Keypair {
    fn from(value: &Keypair) -> Self {
        value.inner
    }
}

impl AsRef<secp256k1::Keypair> for Keypair {
    fn as_ref(&self) -> &secp256k1::Keypair {
        &self.inner
    }
}

#[uniffi::export]
impl Keypair {
    /// See [`secp256k1::Keypair::from_seckey_slice`].
    #[uniffi::constructor]
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let secp = Secp256k1::new();
        let inner = secp256k1::Keypair::from_seckey_slice(&secp, bytes)?;
        Ok(Arc::new(Keypair { inner }))
    }

    /// Create a `Keypair` from a `SecretKey`.
    #[uniffi::constructor]
    pub fn from_secret_key(secret_key: &SecretKey) -> Arc<Self> {
        let secp = Secp256k1::new();
        let sk: secp256k1::SecretKey = secret_key.into();
        let inner = secp256k1::Keypair::from_secret_key(&secp, &sk);
        Arc::new(Keypair { inner })
    }

    /// Generate a new random keypair.
    #[uniffi::constructor]
    pub fn generate() -> Arc<Self> {
        let secp = Secp256k1::new();
        let inner = secp256k1::Keypair::new(&secp, &mut secp256k1::rand::thread_rng());
        Arc::new(Keypair { inner })
    }

    /// Returns the secret key bytes (32 bytes).
    pub fn secret_bytes(&self) -> Vec<u8> {
        self.inner.secret_bytes().to_vec()
    }

    /// Returns the `SecretKey`.
    pub fn secret_key(&self) -> Arc<SecretKey> {
        Arc::new(self.inner.secret_key().into())
    }

    /// Returns the `PublicKey`.
    pub fn public_key(&self) -> Arc<PublicKey> {
        let pk = elements::bitcoin::PublicKey::new(self.inner.public_key());
        Arc::new(pk.into())
    }

    /// Returns the `XOnlyPublicKey`.
    pub fn x_only_public_key(&self) -> XOnlyPublicKey {
        let (xonly, _parity) = self.inner.x_only_public_key();
        xonly.into()
    }
}

#[cfg(feature = "simplicity")]
impl Keypair {
    /// Convert to simplicityhl's Keypair type.
    ///
    /// TODO: delete when the version of elements is stabilized
    pub fn to_simplicityhl(
        &self,
    ) -> Result<lwk_simplicity_options::simplicityhl::elements::bitcoin::secp256k1::Keypair, LwkError>
    {
        let secp =
            lwk_simplicity_options::simplicityhl::elements::bitcoin::secp256k1::Secp256k1::new();
        lwk_simplicity_options::simplicityhl::elements::bitcoin::secp256k1::Keypair::from_seckey_slice(
            &secp,
            &self.secret_bytes(),
        )
        .map_err(|e| LwkError::Generic {
            msg: format!("Invalid keypair: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Keypair, SecretKey};

    #[test]
    fn test_keypair_from_secret_bytes() {
        let bytes = [1u8; 32];
        let kp = Keypair::from_secret_bytes(&bytes).unwrap();
        assert_eq!(kp.secret_bytes(), bytes);
    }

    #[test]
    fn test_keypair_from_secret_key() {
        let sk = SecretKey::from_bytes(&[1u8; 32]).unwrap();
        let kp = Keypair::from_secret_key(&sk);
        assert_eq!(kp.secret_bytes(), sk.bytes());
    }

    #[test]
    fn test_keypair_generate() {
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        assert_ne!(kp1.secret_bytes(), kp2.secret_bytes());
    }

    #[test]
    fn test_keypair_public_key() {
        let kp = Keypair::from_secret_bytes(&[1u8; 32]).unwrap();
        let pk = kp.public_key();
        assert_eq!(pk.to_bytes().len(), 33);
    }

    #[test]
    fn test_keypair_x_only_public_key() {
        let kp = Keypair::from_secret_bytes(&[1u8; 32]).unwrap();
        let xonly = kp.x_only_public_key();
        assert_eq!(xonly.to_string().len(), 64);
    }

    #[test]
    fn test_keypair_secret_key() {
        let bytes = [1u8; 32];
        let kp = Keypair::from_secret_bytes(&bytes).unwrap();
        let sk = kp.secret_key();
        assert_eq!(sk.bytes(), bytes);
    }
}
