use crate::Error;

use std::str::FromStr;

use lwk_wollet::elements;
use lwk_wollet::elements::hashes::Hash;
use lwk_wollet::elements::hex::ToHex;

use wasm_bindgen::prelude::*;

/// The hash of an asset contract.
///
/// See [`elements::ContractHash`] for more details.
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

#[wasm_bindgen]
impl ContractHash {
    /// Creates a `ContractHash` from a hex string.
    #[wasm_bindgen(constructor)]
    pub fn new(hex: &str) -> Result<ContractHash, Error> {
        let inner = elements::ContractHash::from_str(hex)?;
        Ok(ContractHash { inner })
    }

    /// Creates a `ContractHash` from a byte slice.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<ContractHash, Error> {
        let array: [u8; 32] = bytes
            .try_into()
            .map_err(|_| Error::Generic(format!("expected 32 bytes, got {}", bytes.len())))?;
        Ok(ContractHash {
            inner: elements::ContractHash::from_byte_array(array),
        })
    }

    /// Returns the bytes (32 bytes).
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.to_byte_array().to_vec()
    }

    /// Returns the hex-encoded representation.
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.to_hex()
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
        let ch = ContractHash::new(hex).unwrap();
        assert_eq!(ch.to_hex(), hex);

        let bytes = ch.bytes();
        let ch2 = ContractHash::from_bytes(&bytes).unwrap();
        assert_eq!(ch, ch2);

        assert!(ContractHash::new("invalid").is_err());
        assert!(ContractHash::from_bytes(&[0u8; 16]).is_err());
    }
}
