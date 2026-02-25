use crate::LwkError;

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::pset::serialize::Serialize;

/// A blinding factor for asset commitments.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
#[uniffi::export(Display)]
pub struct AssetBlindingFactor {
    inner: elements::confidential::AssetBlindingFactor,
}

impl From<elements::confidential::AssetBlindingFactor> for AssetBlindingFactor {
    fn from(inner: elements::confidential::AssetBlindingFactor) -> Self {
        AssetBlindingFactor { inner }
    }
}

impl From<AssetBlindingFactor> for elements::confidential::AssetBlindingFactor {
    fn from(value: AssetBlindingFactor) -> Self {
        value.inner
    }
}

impl From<&AssetBlindingFactor> for elements::confidential::AssetBlindingFactor {
    fn from(value: &AssetBlindingFactor) -> Self {
        value.inner
    }
}

impl AsRef<elements::confidential::AssetBlindingFactor> for AssetBlindingFactor {
    fn as_ref(&self) -> &elements::confidential::AssetBlindingFactor {
        &self.inner
    }
}

impl FromStr for AssetBlindingFactor {
    type Err = LwkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = elements::confidential::AssetBlindingFactor::from_str(s)?;
        Ok(Self { inner })
    }
}

impl Display for AssetBlindingFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl AssetBlindingFactor {
    /// Create from bytes.
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::confidential::AssetBlindingFactor::from_slice(bytes)?;
        Ok(Arc::new(AssetBlindingFactor { inner }))
    }

    /// Creates from a hex string.
    #[uniffi::constructor]
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(s)?))
    }

    /// Get a unblinded/zero asset blinding factor
    #[uniffi::constructor]
    pub fn zero() -> Arc<Self> {
        Arc::new(AssetBlindingFactor {
            inner: elements::confidential::AssetBlindingFactor::zero(),
        })
    }

    /// Returns the bytes (32 bytes).
    pub fn to_bytes(self) -> Vec<u8> {
        self.inner.into_inner().serialize()
    }
}

/// A blinding factor for value commitments.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
#[uniffi::export(Display)]
pub struct ValueBlindingFactor {
    inner: elements::confidential::ValueBlindingFactor,
}

impl From<elements::confidential::ValueBlindingFactor> for ValueBlindingFactor {
    fn from(inner: elements::confidential::ValueBlindingFactor) -> Self {
        ValueBlindingFactor { inner }
    }
}

impl From<ValueBlindingFactor> for elements::confidential::ValueBlindingFactor {
    fn from(value: ValueBlindingFactor) -> Self {
        value.inner
    }
}

impl From<&ValueBlindingFactor> for elements::confidential::ValueBlindingFactor {
    fn from(value: &ValueBlindingFactor) -> Self {
        value.inner
    }
}

impl AsRef<elements::confidential::ValueBlindingFactor> for ValueBlindingFactor {
    fn as_ref(&self) -> &elements::confidential::ValueBlindingFactor {
        &self.inner
    }
}

impl FromStr for ValueBlindingFactor {
    type Err = LwkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = elements::confidential::ValueBlindingFactor::from_str(s)?;
        Ok(Self { inner })
    }
}

impl Display for ValueBlindingFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl ValueBlindingFactor {
    /// Create from bytes.
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::confidential::ValueBlindingFactor::from_slice(bytes)?;
        Ok(Arc::new(ValueBlindingFactor { inner }))
    }

    /// Creates from a hex string.
    #[uniffi::constructor]
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(s)?))
    }

    /// Get a unblinded/zero value blinding factor
    #[uniffi::constructor]
    pub fn zero() -> Arc<Self> {
        Arc::new(ValueBlindingFactor {
            inner: elements::confidential::ValueBlindingFactor::zero(),
        })
    }

    /// Returns the bytes (32 bytes).
    pub fn to_bytes(self) -> Vec<u8> {
        self.inner.into_inner().serialize()
    }
}

#[cfg(feature = "simplicity")]
impl AssetBlindingFactor {
    /// Convert to simplicityhl's AssetBlindingFactor type.
    ///
    /// TODO: delete when the version of elements is stabilized
    pub fn to_simplicityhl(
        self,
    ) -> Result<lwk_simplicity::simplicityhl::elements::confidential::AssetBlindingFactor, LwkError>
    {
        Ok(
            lwk_simplicity::simplicityhl::elements::confidential::AssetBlindingFactor::from_slice(
                &self.to_bytes(),
            )?,
        )
    }
}

#[cfg(feature = "simplicity")]
impl ValueBlindingFactor {
    /// Convert to simplicityhl's ValueBlindingFactor type.
    ///
    /// TODO: delete when the version of elements is stabilized
    pub fn to_simplicityhl(
        self,
    ) -> Result<lwk_simplicity::simplicityhl::elements::confidential::ValueBlindingFactor, LwkError>
    {
        Ok(
            lwk_simplicity::simplicityhl::elements::confidential::ValueBlindingFactor::from_slice(
                &self.to_bytes(),
            )?,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetBlindingFactor, ValueBlindingFactor};

    #[test]
    fn test_asset_blinding_factor_from_bytes_and_roundtrip() {
        let abf = AssetBlindingFactor::zero();
        assert_eq!(abf.to_bytes(), vec![0u8; 32]);

        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";

        let from_hex = AssetBlindingFactor::from_string(hex).unwrap();
        let from_bytes = AssetBlindingFactor::from_bytes(&from_hex.to_bytes()).unwrap();
        assert_eq!(from_bytes.to_bytes(), from_hex.to_bytes());
        assert_eq!(from_bytes.to_string(), from_hex.to_string());
        assert_eq!(from_hex.to_string(), hex);

        assert!(AssetBlindingFactor::from_bytes(&[0u8; 31]).is_err());
        assert!(AssetBlindingFactor::from_bytes(&[0u8; 33]).is_err());
    }

    #[test]
    fn test_value_blinding_factor_from_bytes_and_roundtrip() {
        let vbf = ValueBlindingFactor::zero();
        assert_eq!(vbf.to_bytes(), vec![0u8; 32]);

        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";

        let from_hex = ValueBlindingFactor::from_string(hex).unwrap();
        let from_bytes = ValueBlindingFactor::from_bytes(&from_hex.to_bytes()).unwrap();
        assert_eq!(from_bytes.to_bytes(), from_hex.to_bytes());
        assert_eq!(from_bytes.to_string(), from_hex.to_string());
        assert_eq!(from_hex.to_string(), hex);

        assert!(ValueBlindingFactor::from_bytes(&[0u8; 31]).is_err());
        assert!(ValueBlindingFactor::from_bytes(&[0u8; 33]).is_err());
    }
}
