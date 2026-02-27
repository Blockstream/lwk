use crate::LwkError;

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::hashes::Hash;

/// The hash of an asset contract.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
#[uniffi::export(Display)]
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
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(s)?))
    }

    /// Creates from a 32-byte slice.
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(ContractHash {
            inner: elements::ContractHash::from_byte_array(bytes.try_into()?),
        }))
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
    fn test_contract_hash_from_bytes_and_roundtrip() {
        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";

        let from_hex = ContractHash::from_string(hex).unwrap();
        assert_eq!(from_hex.to_string(), hex);

        let from_bytes = ContractHash::from_bytes(&from_hex.to_bytes()).unwrap();
        assert_eq!(from_bytes.to_bytes(), from_hex.to_bytes());

        assert!(ContractHash::from_bytes(&[0u8; 31]).is_err());
        assert!(ContractHash::from_bytes(&[0u8; 33]).is_err());
    }
}
