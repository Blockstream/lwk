use crate::Error;
use lwk_signer::bip39;
use std::{fmt::Display, str::FromStr};
use wasm_bindgen::prelude::*;

/// Wrapper of [`bip39::Mnemonic`]
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug)]
pub struct Mnemonic {
    inner: bip39::Mnemonic,
}

impl From<bip39::Mnemonic> for Mnemonic {
    fn from(inner: bip39::Mnemonic) -> Self {
        Self { inner }
    }
}

impl Display for Mnemonic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Mnemonic {
    /// Creates a Mnemonic
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Result<Mnemonic, Error> {
        let inner = bip39::Mnemonic::from_str(s)?;
        Ok(inner.into())
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }
}

#[cfg(test)]
mod tests {
    use crate::Mnemonic;
    use lwk_signer::bip39;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn mnemonic() {
        let mnemonic_str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic_bip39 = bip39::Mnemonic::from_str(mnemonic_str).unwrap();
        let from_bip39: Mnemonic = mnemonic_bip39.into();
        let mnemonic = Mnemonic::new(mnemonic_str).unwrap();
        assert_eq!(mnemonic_str, mnemonic.to_string());
        assert_eq!(from_bip39, mnemonic);
    }
}
