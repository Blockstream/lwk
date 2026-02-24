use crate::types::Hex;
use crate::LwkError;

use std::str::FromStr;
use std::sync::Arc;

use lwk_simplicity::simplicityhl::simplicity;

/// Commitment Merkle root
///
/// A Merkle root that commits to a node's combinator and recursively its children.
#[derive(uniffi::Object, Clone, Debug)]
pub struct Cmr {
    inner: simplicity::Cmr,
}

impl From<simplicity::Cmr> for Cmr {
    fn from(inner: simplicity::Cmr) -> Self {
        Self { inner }
    }
}

impl Cmr {
    pub(crate) fn inner(&self) -> simplicity::Cmr {
        self.inner
    }
}

#[uniffi::export]
impl Cmr {
    /// Create from raw bytes (32 bytes).
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: simplicity::Cmr::from_byte_array(bytes.try_into()?),
        }))
    }

    /// Create from hex (64 hex characters = 32 bytes).
    #[uniffi::constructor]
    pub fn from_hex(hex: Hex) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(
            simplicity::Cmr::from_str(&hex.to_string())?.into(),
        ))
    }

    /// Return the hex-encoded CMR.
    pub fn to_hex(&self) -> Result<Hex, LwkError> {
        Ok(Hex::from_str(&self.inner.to_string())?)
    }

    /// Return the raw CMR bytes (32 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_byte_array().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::simplicity::{SimplicityArguments, SimplicityProgram, SimplicityTypedValue};

    const TEST_CMR: &str = "b685a4424842507d7d747e6611a740d8c421038e9744e75d423d0e2e9f164d02";
    const TEST_PUBLIC_KEY: &str =
        "8a65c55726dc32b59b649ad0187eb44490de681bb02601b8d3f58c8b9fff9083";
    const P2PK_SOURCE: &str = include_str!("../../../lwk_simplicity/data/p2pk.simf");

    #[test]
    fn test_cmr_hex_validation_and_roundtrip() {
        let expected_hex = Hex::from_str(TEST_CMR).unwrap();
        let expected_bytes = Hex::from_str(TEST_CMR).unwrap().as_ref().to_vec();

        let cmr = Cmr::from_hex(expected_hex.clone()).unwrap();
        assert_eq!(cmr.to_hex().unwrap(), expected_hex);
        assert_eq!(cmr.to_bytes(), expected_bytes);

        let from_bytes = Cmr::from_bytes(&expected_bytes).unwrap();
        assert_eq!(from_bytes.to_hex().unwrap(), expected_hex);
        assert_eq!(from_bytes.to_bytes(), expected_bytes);

        assert!(Cmr::from_bytes(&[0u8; 31]).is_err());
        assert!(Cmr::from_bytes(&[0u8; 33]).is_err());
        assert!(Cmr::from_bytes(&[]).is_err());
        assert!(Cmr::from_hex(Hex::from_str("0011").unwrap()).is_err());

        let args = SimplicityArguments::new().add_value(
            "PUBLIC_KEY".to_string(),
            &SimplicityTypedValue::u256(Hex::from_str(TEST_PUBLIC_KEY).unwrap()).unwrap(),
        );
        let program = SimplicityProgram::load(P2PK_SOURCE.to_string(), &args).unwrap();
        let cmr = program.cmr();
        assert_eq!(cmr.to_hex().unwrap(), expected_hex);
        assert_eq!(cmr.to_bytes(), expected_bytes);
    }
}
