//! sha256::Midstate wrapper

use crate::types::Hex;
use crate::LwkError;

use std::str::FromStr;
use std::sync::Arc;

use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::hashes::hex::FromHex;
use lwk_wollet::hashes::sha256;

/// Output of the SHA256 hash function.
///
/// See [`sha256::Midstate`] for more details.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
pub struct Midstate {
    pub(crate) inner: sha256::Midstate,
}

impl From<sha256::Midstate> for Midstate {
    fn from(inner: sha256::Midstate) -> Self {
        Self { inner }
    }
}

impl From<Midstate> for sha256::Midstate {
    fn from(value: Midstate) -> Self {
        value.inner
    }
}

#[uniffi::export]
impl Midstate {
    /// Create a midstate from hex (64 hex characters = 32 bytes).
    #[uniffi::constructor]
    pub fn new(hex: Hex) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: sha256::Midstate::from_hex(&hex.to_string())?,
        }))
    }

    /// Create a midstate from bytes
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| LwkError::Generic {
            msg: format!("expected 32 bytes, got {}", bytes.len()),
        })?;
        Ok(Arc::new(Self {
            inner: sha256::Midstate::from_byte_array(bytes),
        }))
    }

    /// Return the hex representation.
    pub fn to_hex(&self) -> Result<Hex, LwkError> {
        Ok(Hex::from_str(&self.inner.to_hex())?)
    }

    /// Return the raw bytes
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.to_byte_array().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midstate_roundtrip() {
        let hex = Hex::from_str("0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")
            .unwrap();
        let bytes = hex.clone().as_ref().to_vec();

        let from_hex = Midstate::new(hex.clone()).unwrap();
        assert_eq!(from_hex.to_hex().unwrap(), hex);

        let from_bytes = Midstate::from_bytes(&bytes).unwrap();
        assert_eq!(from_bytes.bytes(), bytes.to_vec());
    }
}
