use crate::Error;

use std::fmt::Display;
use std::str::FromStr;

use lwk_wollet::elements;
use lwk_wollet::elements::hashes::Hash;

use wasm_bindgen::prelude::*;

/// The hash of an asset contract.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
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

impl Display for ContractHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl ContractHash {
    /// Creates a `ContractHash` from a string.
    #[wasm_bindgen(js_name = fromString)]
    pub fn from_string(s: &str) -> Result<ContractHash, Error> {
        let inner = elements::ContractHash::from_str(s)?;
        Ok(ContractHash { inner })
    }

    /// Creates a `ContractHash` from a byte slice.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<ContractHash, Error> {
        Ok(ContractHash {
            inner: elements::ContractHash::from_byte_array(bytes.try_into()?),
        })
    }

    /// Returns the bytes (32 bytes).
    #[wasm_bindgen(js_name = toBytes)]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.to_byte_array().to_vec()
    }

    /// Returns the string representation.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_contract_hash() {
        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let ch = ContractHash::from_string(hex).unwrap();
        assert_eq!(ch.to_string(), hex);

        let bytes = ch.to_bytes();
        let ch2 = ContractHash::from_bytes(&bytes).unwrap();
        assert_eq!(ch, ch2);

        assert!(ContractHash::from_string("invalid").is_err());
        assert!(ContractHash::from_bytes(&[0u8; 16]).is_err());
    }
}
