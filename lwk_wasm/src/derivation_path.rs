use crate::Error;
use lwk_wollet::bitcoin::bip32::{self, ChildNumber};
use std::{fmt, str::FromStr};

use wasm_bindgen::prelude::*;

/// A BIP32 derivation path
#[wasm_bindgen]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivationPath {
    inner: bip32::DerivationPath,
}

impl From<bip32::DerivationPath> for DerivationPath {
    fn from(inner: bip32::DerivationPath) -> Self {
        Self { inner }
    }
}

impl From<DerivationPath> for bip32::DerivationPath {
    fn from(value: DerivationPath) -> Self {
        value.inner
    }
}

impl From<&DerivationPath> for bip32::DerivationPath {
    fn from(value: &DerivationPath) -> Self {
        value.inner.clone()
    }
}

impl AsRef<bip32::DerivationPath> for DerivationPath {
    fn as_ref(&self) -> &bip32::DerivationPath {
        &self.inner
    }
}

impl fmt::Display for DerivationPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl DerivationPath {
    /// Construct a DerivationPath from its string representation.
    ///
    /// For example: "m/84'/1'/0'" or "84h/1h/0h".
    #[wasm_bindgen(constructor)]
    pub fn new(path: &str) -> Result<DerivationPath, Error> {
        let inner = bip32::DerivationPath::from_str(path)?;
        Ok(Self { inner })
    }

    /// Construct a DerivationPath from a vector of u32
    #[wasm_bindgen(js_name = fromVec)]
    pub fn from_vec(path: Vec<u32>) -> DerivationPath {
        let inner = path.into_iter().map(ChildNumber::from).collect();
        Self { inner }
    }

    /// Return the derivation path as a vector of u32
    #[wasm_bindgen(js_name = toVec)]
    pub fn to_vec(&self) -> Vec<u32> {
        self.inner.into_iter().map(|c| u32::from(*c)).collect()
    }

    /// Return the string representation of the derivation path.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::DerivationPath;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_derivation_path() {
        let s = "84'/1'/0'";
        let path = DerivationPath::new(s).unwrap();
        assert_eq!(path.to_string_js(), s);
        assert_eq!(path.to_vec(), vec![84 + (1 << 31), 1 + (1 << 31), 1 << 31]);

        let from_vec = DerivationPath::from_vec(path.to_vec());
        assert_eq!(from_vec.to_string_js(), s);
        assert_eq!(from_vec, path);

        assert!(DerivationPath::new("not a path").is_err());
    }
}
