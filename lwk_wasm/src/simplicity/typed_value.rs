//! Typed Simplicity values.

use crate::Error;

use super::simplicity_type::SimplicityType;
use super::utils::{hex_to_bytes, hex_to_bytes_32};

use lwk_simplicity::simplicityhl::num::U256;
use lwk_simplicity::simplicityhl::value::ValueConstructible;
use lwk_simplicity::simplicityhl::Value;

use wasm_bindgen::prelude::*;

/// Typed Simplicity value.
///
/// See [`Value`] for more details.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct SimplicityTypedValue {
    inner: Value,
}

#[wasm_bindgen]
impl SimplicityTypedValue {
    /// Create a `u32` value.
    #[wasm_bindgen(js_name = fromU32)]
    pub fn from_u32(value: u32) -> SimplicityTypedValue {
        Self {
            inner: Value::u32(value),
        }
    }

    /// Create a `u64` value.
    #[wasm_bindgen(js_name = fromU64)]
    pub fn from_u64(value: u64) -> SimplicityTypedValue {
        Self {
            inner: Value::u64(value),
        }
    }

    /// Create a `u256` value from hex (64 hex characters = 32 bytes).
    #[wasm_bindgen(js_name = fromU256Hex)]
    pub fn from_u256_hex(hex: &str) -> Result<SimplicityTypedValue, Error> {
        let arr = hex_to_bytes_32(hex)?;
        Ok(Self {
            inner: Value::u256(U256::from_byte_array(arr)),
        })
    }

    /// Create a `bool` value.
    #[wasm_bindgen(js_name = fromBoolean)]
    pub fn from_boolean(value: bool) -> SimplicityTypedValue {
        Self {
            inner: Value::from(value),
        }
    }

    /// Create a `Left` value.
    pub fn left(value: &SimplicityTypedValue, right_type: &SimplicityType) -> SimplicityTypedValue {
        Self {
            inner: Value::left(value.inner.clone(), right_type.inner().clone()),
        }
    }

    /// Create a `Right` value.
    pub fn right(left_type: &SimplicityType, value: &SimplicityTypedValue) -> SimplicityTypedValue {
        Self {
            inner: Value::right(left_type.inner().clone(), value.inner.clone()),
        }
    }

    /// Create a tuple value from elements.
    #[wasm_bindgen(js_name = fromElements)]
    pub fn from_elements(elements: Vec<SimplicityTypedValue>) -> SimplicityTypedValue {
        let inner = Value::tuple(elements.iter().map(|e| e.inner.clone()));
        Self { inner }
    }

    /// Create a `None` value.
    pub fn none(inner_type: &SimplicityType) -> SimplicityTypedValue {
        Self {
            inner: Value::none(inner_type.inner().clone()),
        }
    }

    /// Create a `Some` value.
    pub fn some(value: &SimplicityTypedValue) -> SimplicityTypedValue {
        Self {
            inner: Value::some(value.inner.clone()),
        }
    }

    /// Create a byte array value from hex.
    #[wasm_bindgen(js_name = fromByteArrayHex)]
    pub fn from_byte_array_hex(hex: &str) -> Result<SimplicityTypedValue, Error> {
        Ok(Self {
            inner: Value::byte_array(hex_to_bytes(hex)?),
        })
    }

    /// Parse a value from a string with a given type.
    #[wasm_bindgen(constructor)]
    pub fn new(value_str: &str, ty: &SimplicityType) -> Result<SimplicityTypedValue, Error> {
        let inner = Value::parse_from_str(value_str, ty.inner())?;
        Ok(Self { inner })
    }
}

impl SimplicityTypedValue {
    pub(crate) fn inner(&self) -> &Value {
        &self.inner
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_simplicity_typed_value() {
        let _ = SimplicityTypedValue::from_u32(42);
        let _ = SimplicityTypedValue::from_u64(1000000);
        let _ = SimplicityTypedValue::from_boolean(true);
        let _ = SimplicityTypedValue::from_boolean(false);

        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let _ = SimplicityTypedValue::from_u256_hex(hex).unwrap();
        assert!(SimplicityTypedValue::from_u256_hex("0011").is_err());

        let u32_type = SimplicityType::u32();
        let u64_type = SimplicityType::u64();

        let val_32 = SimplicityTypedValue::from_u32(42);
        let val_64 = SimplicityTypedValue::from_u64(1000);

        let _ = SimplicityTypedValue::left(&val_32, &u64_type);
        let _ = SimplicityTypedValue::right(&u32_type, &val_64);
        let _ = SimplicityTypedValue::none(&u32_type);
        let _ = SimplicityTypedValue::some(&val_32);
        let _ = SimplicityTypedValue::from_elements(vec![val_32, val_64]);

        let _ = SimplicityTypedValue::from_byte_array_hex("deadbeef").unwrap();

        let ty = SimplicityType::u32();
        let _ = SimplicityTypedValue::new("42", &ty).unwrap();
    }
}
