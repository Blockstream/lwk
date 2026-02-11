//! sha256::Midstate wrapper

use crate::Error;

use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::hashes::hex::FromHex;
use lwk_wollet::hashes::sha256;

use wasm_bindgen::prelude::*;

/// Output of the SHA256 hash function.
///
/// See [`sha256::Midstate`] for more details.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Midstate {
    pub(crate) inner: sha256::Midstate,
}

impl From<sha256::Midstate> for Midstate {
    fn from(inner: sha256::Midstate) -> Self {
        Self { inner }
    }
}

impl From<Midstate> for sha256::Midstate {
    fn from(value: Midstate) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl Midstate {
    /// Create a midstate from hex (64 hex characters = 32 bytes).
    #[wasm_bindgen(constructor)]
    pub fn new(hex: &str) -> Result<Midstate, Error> {
        Ok(Midstate {
            inner: sha256::Midstate::from_hex(hex)?,
        })
    }

    /// Create a midstate from bytes (32 bytes).
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Midstate, Error> {
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| Error::Generic(format!("expected 32 bytes, got {}", bytes.len())))?;
        Ok(Midstate {
            inner: sha256::Midstate::from_byte_array(bytes),
        })
    }

    /// Return the hex representation (64 hex characters).
    #[wasm_bindgen(js_name = toHex)]
    pub fn to_hex(&self) -> String {
        self.inner.to_hex()
    }

    /// Return the raw bytes (32 bytes).
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.to_byte_array().to_vec()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_midstate_roundtrip() {
        let hex = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
        let bytes = <[u8; 32]>::from_hex(hex).unwrap();

        let from_hex = Midstate::new(hex).unwrap();
        assert_eq!(from_hex.to_hex(), hex);

        let from_bytes = Midstate::from_bytes(&bytes).unwrap();
        assert_eq!(from_bytes.bytes(), bytes.to_vec());
    }
}
