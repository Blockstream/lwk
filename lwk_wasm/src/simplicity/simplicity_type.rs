use crate::Error;

use std::fmt::Display;

use lwk_simplicity::simplicityhl::parse::ParseFromStr;
use lwk_simplicity::simplicityhl::types::TypeConstructible;
use lwk_simplicity::simplicityhl::ResolvedType;

use wasm_bindgen::prelude::*;

/// Simplicity type descriptor.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct SimplicityType {
    inner: ResolvedType,
}

impl Display for SimplicityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl SimplicityType {
    /// Create the `u1` type.
    pub fn u1() -> SimplicityType {
        Self {
            inner: ResolvedType::u1(),
        }
    }

    /// Create the `u8` type.
    pub fn u8() -> SimplicityType {
        Self {
            inner: ResolvedType::u8(),
        }
    }

    /// Create the `u16` type.
    pub fn u16() -> SimplicityType {
        Self {
            inner: ResolvedType::u16(),
        }
    }

    /// Create the `u32` type.
    pub fn u32() -> SimplicityType {
        Self {
            inner: ResolvedType::u32(),
        }
    }

    /// Create the `u64` type.
    pub fn u64() -> SimplicityType {
        Self {
            inner: ResolvedType::u64(),
        }
    }

    /// Create the `u128` type.
    pub fn u128() -> SimplicityType {
        Self {
            inner: ResolvedType::u128(),
        }
    }

    /// Create the `u256` type.
    pub fn u256() -> SimplicityType {
        Self {
            inner: ResolvedType::u256(),
        }
    }

    /// Create the `bool` type.
    pub fn boolean() -> SimplicityType {
        Self {
            inner: ResolvedType::boolean(),
        }
    }

    /// Create an `Either<left, right>` type.
    pub fn either(left: &SimplicityType, right: &SimplicityType) -> SimplicityType {
        Self {
            inner: ResolvedType::either(left.inner.clone(), right.inner.clone()),
        }
    }

    /// Create an `Option<inner>` type.
    pub fn option(inner: &SimplicityType) -> SimplicityType {
        Self {
            inner: ResolvedType::option(inner.inner.clone()),
        }
    }

    /// Create a tuple type from elements.
    ///
    /// NOTE: The elements object is destroyed during the execution of the function, so the argument that was
    /// passed in the JS code cannot be reused.
    // TODO: address the limitation
    #[wasm_bindgen(js_name = fromElements)]
    pub fn from_elements(elements: Vec<SimplicityType>) -> SimplicityType {
        let inner = ResolvedType::tuple(elements.iter().map(|e| e.inner.clone()));
        Self { inner }
    }

    /// Parse a type from a string.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(type_str: &str) -> Result<SimplicityType, Error> {
        let inner = ResolvedType::parse_from_str(type_str)?;
        Ok(Self { inner })
    }

    /// Return the canonical string representation of the type.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

impl SimplicityType {
    pub(crate) fn inner(&self) -> &ResolvedType {
        &self.inner
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_simplicity_type() {
        let _ = SimplicityType::u1();
        let _ = SimplicityType::u8();
        let _ = SimplicityType::u16();
        let _ = SimplicityType::u32();
        let _ = SimplicityType::u64();
        let _ = SimplicityType::u128();
        let _ = SimplicityType::u256();
        let _ = SimplicityType::boolean();

        let u32_type = SimplicityType::u32();
        let u64_type = SimplicityType::u64();

        let _ = SimplicityType::either(&u32_type, &u64_type);
        let _ = SimplicityType::option(&u32_type);
        let _ = SimplicityType::from_elements(vec![u32_type, u64_type]);

        let ty = SimplicityType::from_string("u32").unwrap();
        let either_ty = SimplicityType::from_string("Either<u32, u64>").unwrap();

        assert!(SimplicityType::from_string("invalid_type").is_err());
    }
}
