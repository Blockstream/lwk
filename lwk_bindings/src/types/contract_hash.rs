use crate::LwkError;

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::hashes::Hash;
use elements::hex::ToHex;

/// The hash of an asset contract.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
pub struct ContractHash {
    inner: elements::ContractHash,
}

impl From<elements::ContractHash> for ContractHash {
    fn from(inner: elements::ContractHash) -> Self {
        ContractHash { inner }
    }
}

impl From<ContractHash> for elements::ContractHash {
    fn from(value: ContractHash) -> Self {
        value.inner
    }
}

impl From<&ContractHash> for elements::ContractHash {
    fn from(value: &ContractHash) -> Self {
        value.inner
    }
}

impl AsRef<elements::ContractHash> for ContractHash {
    fn as_ref(&self) -> &elements::ContractHash {
        &self.inner
    }
}

impl FromStr for ContractHash {
    type Err = LwkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = elements::ContractHash::from_str(s)?;
        Ok(Self { inner })
    }
}

impl Display for ContractHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl ContractHash {
    /// Creates from a hex string.
    #[uniffi::constructor]
    pub fn from_hex(hex: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(hex)?))
    }

    /// Creates from a 32-byte slice.
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let array: [u8; 32] = bytes.try_into().map_err(|_| LwkError::Generic {
            msg: format!("expected 32 bytes, got {}", bytes.len()),
        })?;
        Ok(Arc::new(ContractHash {
            inner: elements::ContractHash::from_byte_array(array),
        }))
    }

    /// Returns the hex-encoded representation.
    pub fn to_hex(&self) -> String {
        self.inner.to_hex()
    }

    /// Returns the bytes (32 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_byte_array().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::ContractHash;

    #[test]
    fn test_contract_hash_hex_roundtrip() {
        let hex = "0101010101010101010101010101010101010101010101010101010101010101";
        let ch = ContractHash::from_hex(hex).unwrap();
        assert_eq!(ch.to_hex(), hex);
    }

    #[test]
    fn test_contract_hash_bytes_roundtrip() {
        let bytes = [42u8; 32];
        let ch = ContractHash::from_bytes(&bytes).unwrap();
        assert_eq!(ch.to_bytes(), bytes);
    }

    #[test]
    fn test_contract_hash_from_bytes_invalid_length() {
        let bytes = [0u8; 16];
        assert!(ContractHash::from_bytes(&bytes).is_err());
    }
}
