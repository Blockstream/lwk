use crate::Error;
use lwk_wollet::bitcoin::bip32;
use std::{fmt::Display, str::FromStr};
use wasm_bindgen::prelude::*;

/// An extended public key
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug)]
pub struct Xpub {
    inner: bip32::Xpub,
}

impl From<bip32::Xpub> for Xpub {
    fn from(inner: bip32::Xpub) -> Self {
        Self { inner }
    }
}

impl Display for Xpub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Xpub {
    /// Creates a Xpub
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Result<Xpub, Error> {
        let inner = bip32::Xpub::from_str(s)?;
        Ok(inner.into())
    }

    /// Return the string representation of the Xpub.
    /// This representation can be used to recreate the Xpub via `new()`
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    /// Return the identifier of the Xpub.
    /// This is a 40 hex characters string (20 bytes).
    pub fn identifier(&self) -> String {
        self.inner.identifier().to_string()
    }

    /// Return the first four bytes of the identifier as hex string
    /// This is a 8 hex characters string (4 bytes).
    pub fn fingerprint(&self) -> String {
        self.inner.fingerprint().to_string()
    }

    /// Returns true if the passed string is a valid xpub with a valid keyorigin if present.
    /// For example: "[73c5da0a/84h/1h/0h]tpub..."
    #[wasm_bindgen(js_name = isValidWithKeyOrigin)]
    pub fn is_valid_with_keyorigin(s: &str) -> bool {
        lwk_common::keyorigin_xpub_from_str(s).is_ok()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::Xpub;
    use lwk_wollet::bitcoin::bip32;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn xpub() {
        let xpub_str = "tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87";
        let xpub_bip32 = bip32::Xpub::from_str(xpub_str).unwrap();
        let from_bip32: Xpub = xpub_bip32.into();
        let xpub = Xpub::new(xpub_str).unwrap();
        assert_eq!(xpub_str, xpub.to_string());
        assert_eq!(from_bip32, xpub);
        let expected = "15c918d389673c6cd0660050f268a843361e1111";
        assert_eq!(xpub.identifier(), expected);
        assert_eq!(xpub.fingerprint(), &expected[0..8]);
    }
}
