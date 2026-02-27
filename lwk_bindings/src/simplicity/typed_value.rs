use super::simplicity_type::SimplicityType;
use crate::LwkError;

use std::sync::Arc;

use lwk_simplicity::simplicityhl;
use lwk_simplicity::simplicityhl::num::U256;
use lwk_simplicity::simplicityhl::value::ValueConstructible;

/// Typed Simplicity value.
#[derive(uniffi::Object, Clone, Debug)]
pub struct SimplicityTypedValue {
    inner: simplicityhl::Value,
}

#[uniffi::export]
impl SimplicityTypedValue {
    /// Create a `u8` value.
    #[uniffi::constructor]
    pub fn u8(value: u8) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::u8(value),
        })
    }

    /// Create a `u16` value.
    #[uniffi::constructor]
    pub fn u16(value: u16) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::u16(value),
        })
    }

    /// Create a `u32` value.
    #[uniffi::constructor]
    pub fn u32(value: u32) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::u32(value),
        })
    }

    /// Create a `u64` value.
    #[uniffi::constructor]
    pub fn u64(value: u64) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::u64(value),
        })
    }

    /// Create a `u128` value from hex.
    #[uniffi::constructor]
    pub fn u128(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: simplicityhl::Value::u128(u128::from_be_bytes(bytes.try_into()?)),
        }))
    }

    /// Create a `u256` value from hex.
    #[uniffi::constructor]
    pub fn u256(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: simplicityhl::Value::u256(U256::from_byte_array(bytes.try_into()?)),
        }))
    }

    /// Create a `bool` value.
    #[uniffi::constructor]
    pub fn boolean(value: bool) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::from(value),
        })
    }

    /// Create a `Left` value.
    #[uniffi::constructor]
    pub fn left(value: &SimplicityTypedValue, right_type: &SimplicityType) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::left(value.inner.clone(), right_type.inner().clone()),
        })
    }

    /// Create a `Right` value.
    #[uniffi::constructor]
    pub fn right(left_type: &SimplicityType, value: &SimplicityTypedValue) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::right(left_type.inner().clone(), value.inner.clone()),
        })
    }

    /// Create a tuple value from elements.
    #[uniffi::constructor]
    pub fn tuple(elements: Vec<Arc<SimplicityTypedValue>>) -> Arc<Self> {
        let inner = simplicityhl::Value::tuple(elements.iter().map(|e| e.inner.clone()));
        Arc::new(Self { inner })
    }

    /// Create a `None` value.
    #[uniffi::constructor]
    pub fn none(inner_type: &SimplicityType) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::none(inner_type.inner().clone()),
        })
    }

    /// Create a `Some` value.
    #[uniffi::constructor]
    pub fn some(value: &SimplicityTypedValue) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::Value::some(value.inner.clone()),
        })
    }

    /// Create a byte array value from hex.
    #[uniffi::constructor]
    pub fn byte_array(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: simplicityhl::Value::byte_array(bytes.iter().copied()),
        }))
    }

    /// Parse a value from a string with a given type.
    #[uniffi::constructor]
    pub fn parse(value_str: String, ty: &SimplicityType) -> Result<Arc<Self>, LwkError> {
        let inner = simplicityhl::Value::parse_from_str(&value_str, ty.inner())?;
        Ok(Arc::new(Self { inner }))
    }
}

impl SimplicityTypedValue {
    pub(crate) fn inner(&self) -> &simplicityhl::Value {
        &self.inner
    }
}
