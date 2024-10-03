use crate::LwkError;
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
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = secp256k1::SecretKey::from_slice(bytes)?;
        Ok(Arc::new(SecretKey { inner }))
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.inner.secret_bytes().to_vec()
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
    }
}
