use std::sync::Arc;

use lwk_simplicity::simplicityhl;
use simplicityhl::parse::ParseFromStr;
use simplicityhl::types::TypeConstructible;

use crate::LwkError;

/// Simplicity type descriptor.
///
/// See [`simplicityhl::ResolvedType`] for more details.
#[derive(uniffi::Object, Clone, Debug)]
pub struct SimplicityType {
    inner: simplicityhl::ResolvedType,
}

#[uniffi::export]
impl SimplicityType {
    /// Create the `u1` type.
    #[uniffi::constructor]
    pub fn u1() -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::u1(),
        })
    }

    /// Create the `u8` type.
    #[uniffi::constructor]
    pub fn u8() -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::u8(),
        })
    }

    /// Create the `u16` type.
    #[uniffi::constructor]
    pub fn u16() -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::u16(),
        })
    }

    /// Create the `u32` type.
    #[uniffi::constructor]
    pub fn u32() -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::u32(),
        })
    }

    /// Create the `u64` type.
    #[uniffi::constructor]
    pub fn u64() -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::u64(),
        })
    }

    /// Create the `u128` type.
    #[uniffi::constructor]
    pub fn u128() -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::u128(),
        })
    }

    /// Create the `u256` type.
    #[uniffi::constructor]
    pub fn u256() -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::u256(),
        })
    }

    /// Create the `bool` type.
    #[uniffi::constructor]
    pub fn boolean() -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::boolean(),
        })
    }

    /// Create an `Either<left, right>` type.
    #[uniffi::constructor]
    pub fn either(left: &SimplicityType, right: &SimplicityType) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::either(left.inner.clone(), right.inner.clone()),
        })
    }

    /// Create an `Option<inner>` type.
    #[uniffi::constructor]
    pub fn option(inner: &SimplicityType) -> Arc<Self> {
        Arc::new(Self {
            inner: simplicityhl::ResolvedType::option(inner.inner.clone()),
        })
    }

    /// Create a tuple type from elements.
    #[uniffi::constructor]
    pub fn tuple(elements: Vec<Arc<SimplicityType>>) -> Arc<Self> {
        let inner = simplicityhl::ResolvedType::tuple(elements.iter().map(|e| e.inner.clone()));
        Arc::new(Self { inner })
    }

    /// Parse a type from a string.
    #[uniffi::constructor]
    pub fn parse(type_str: String) -> Result<Arc<Self>, LwkError> {
        let inner = simplicityhl::ResolvedType::parse_from_str(&type_str)?;
        Ok(Arc::new(Self { inner }))
    }
}

impl SimplicityType {
    pub(crate) fn inner(&self) -> &simplicityhl::ResolvedType {
        &self.inner
    }
}
