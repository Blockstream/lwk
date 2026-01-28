use crate::LwkError;

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::hex::ToHex;

/// A blinding factor for asset commitments.
///
/// See [`elements::confidential::AssetBlindingFactor`] for more details.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
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
    /// See [`elements::confidential::AssetBlindingFactor::from_slice`].
    #[uniffi::constructor]
    pub fn from_slice(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::confidential::AssetBlindingFactor::from_slice(bytes)?;
        Ok(Arc::new(AssetBlindingFactor { inner }))
    }

    /// Creates from a hex string.
    #[uniffi::constructor]
    pub fn from_hex(hex: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(hex)?))
    }

    /// See [`elements::confidential::AssetBlindingFactor::zero`].
    #[uniffi::constructor]
    pub fn zero() -> Arc<Self> {
        Arc::new(AssetBlindingFactor {
            inner: elements::confidential::AssetBlindingFactor::zero(),
        })
    }

    /// Returns the bytes (32 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.into_inner().as_ref().to_vec()
    }

    /// Returns the hex-encoded representation.
    pub fn to_hex(&self) -> String {
        self.inner.into_inner().as_ref().to_hex()
    }
}

/// A blinding factor for value commitments.
///
/// See [`elements::confidential::ValueBlindingFactor`] for more details.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone, Copy)]
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
    /// See [`elements::confidential::ValueBlindingFactor::from_slice`].
    #[uniffi::constructor]
    pub fn from_slice(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::confidential::ValueBlindingFactor::from_slice(bytes)?;
        Ok(Arc::new(ValueBlindingFactor { inner }))
    }

    /// Creates from a hex string.
    #[uniffi::constructor]
    pub fn from_hex(hex: &str) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self::from_str(hex)?))
    }

    /// See [`elements::confidential::ValueBlindingFactor::zero`].
    #[uniffi::constructor]
    pub fn zero() -> Arc<Self> {
        Arc::new(ValueBlindingFactor {
            inner: elements::confidential::ValueBlindingFactor::zero(),
        })
    }

    /// Returns the bytes (32 bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.into_inner().as_ref().to_vec()
    }

    /// Returns the hex-encoded representation.
    pub fn to_hex(&self) -> String {
        self.inner.into_inner().as_ref().to_hex()
    }
}

#[cfg(feature = "simplicity")]
impl AssetBlindingFactor {
    /// Convert to simplicityhl's AssetBlindingFactor type.
    ///
    /// TODO: delete when the version of elements is stabilized
    pub fn to_simplicityhl(
        &self,
    ) -> Result<lwk_simplicity::simplicityhl::elements::confidential::AssetBlindingFactor, LwkError>
    {
        lwk_simplicity::simplicityhl::elements::confidential::AssetBlindingFactor::from_slice(
            &self.to_bytes(),
        )
        .map_err(|e| LwkError::Generic {
            msg: format!("Invalid asset blinding factor: {e}"),
        })
    }
}

#[cfg(feature = "simplicity")]
impl ValueBlindingFactor {
    /// Convert to simplicityhl's ValueBlindingFactor type.
    ///
    /// TODO: delete when the version of elements is stabilized
    pub fn to_simplicityhl(
        &self,
    ) -> Result<lwk_simplicity::simplicityhl::elements::confidential::ValueBlindingFactor, LwkError>
    {
        lwk_simplicity::simplicityhl::elements::confidential::ValueBlindingFactor::from_slice(
            &self.to_bytes(),
        )
        .map_err(|e| LwkError::Generic {
            msg: format!("Invalid value blinding factor: {e}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetBlindingFactor, ValueBlindingFactor};

    #[test]
    fn test_asset_blinding_factor_zero() {
        let abf = AssetBlindingFactor::zero();
        assert_eq!(abf.to_bytes(), vec![0u8; 32]);
    }

    #[test]
    fn test_asset_blinding_factor_from_slice() {
        let bytes = [1u8; 32];
        let abf = AssetBlindingFactor::from_slice(&bytes).unwrap();
        assert_eq!(abf.to_bytes(), bytes);
    }

    #[test]
    fn test_asset_blinding_factor_roundtrip() {
        let hex = "0101010101010101010101010101010101010101010101010101010101010101";
        let abf = AssetBlindingFactor::from_hex(hex).unwrap();
        assert_eq!(abf.to_hex(), hex);
    }

    #[test]
    fn test_value_blinding_factor_zero() {
        let vbf = ValueBlindingFactor::zero();
        assert_eq!(vbf.to_bytes(), vec![0u8; 32]);
    }

    #[test]
    fn test_value_blinding_factor_from_slice() {
        let bytes = [2u8; 32];
        let vbf = ValueBlindingFactor::from_slice(&bytes).unwrap();
        assert_eq!(vbf.to_bytes(), bytes);
    }

    #[test]
    fn test_value_blinding_factor_roundtrip() {
        let hex = "0202020202020202020202020202020202020202020202020202020202020202";
        let vbf = ValueBlindingFactor::from_hex(hex).unwrap();
        assert_eq!(vbf.to_hex(), hex);
    }
}
