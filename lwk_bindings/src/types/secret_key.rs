use crate::LwkError;
use crate::Pset;
use elements::bitcoin::secp256k1;
use std::sync::Arc;

/// A secret key
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
pub struct SecretKey {
    inner: secp256k1::SecretKey,
}

impl From<secp256k1::SecretKey> for SecretKey {
    fn from(inner: secp256k1::SecretKey) -> Self {
        SecretKey { inner }
    }
}

impl From<SecretKey> for secp256k1::SecretKey {
    fn from(value: SecretKey) -> Self {
        value.inner
    }
}

impl From<&SecretKey> for secp256k1::SecretKey {
    fn from(value: &SecretKey) -> Self {
        value.inner
    }
}

#[uniffi::export]
impl SecretKey {
    /// Creates a `SecretKey` from a byte array
    ///
    /// The bytes can be used to create a `SecretKey` with `from_bytes()`
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = secp256k1::SecretKey::from_slice(bytes)?;
        Ok(Arc::new(SecretKey { inner }))
    }

    /// Returns the bytes of the secret key, the bytes can be used to create a `SecretKey` with `from_bytes()`
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.secret_bytes().to_vec()
    }

    /// Creates a `SecretKey` from a WIF (Wallet Import Format) string
    #[uniffi::constructor]
    pub fn from_wif(wif: &str) -> Result<Arc<Self>, LwkError> {
        let inner = elements::bitcoin::PrivateKey::from_wif(wif)?.inner;
        Ok(Arc::new(SecretKey { inner }))
    }

    /// Sign the given `pset`
    pub fn sign(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let mut pset = pset.inner();
        lwk_signer::sign_with_seckey(self.inner, &mut pset)?;
        Ok(Arc::new(pset.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::SecretKey;
    use elements::bitcoin::secp256k1;

    #[test]
    fn test_secret_key() {
        let bytes = [0xcd; 32];
        let secp_key = secp256k1::SecretKey::from_slice(&bytes).unwrap();
        let key: SecretKey = secp_key.into();
        let key1 = SecretKey::from_bytes(&bytes).unwrap();

        assert_eq!(key, *key1);
        assert_eq!(&key.bytes(), &bytes);

        let wif_test = "cTJTN1hGHqucsgqmYVbhU3g4eU9g5HzE1sxuSY32M1xap1K4sYHF";
        let wif_main = "L2wTu6hQrnDMiFNWA5na6jB12ErGQqtXwqpSL7aWquJaZG8Ai3ch";
        let key_test = SecretKey::from_wif(wif_test).unwrap();
        let key_main = SecretKey::from_wif(wif_main).unwrap();
        assert_eq!(key_test.bytes(), key_main.bytes());
    }
}
