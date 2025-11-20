use crate::Error;
use lwk_signer::bip39;
use std::{fmt::Display, str::FromStr};
use wasm_bindgen::prelude::*;

/// A mnemonic secret code used as a master secret for a bip39 wallet.
///
/// Supported number of words are 12, 15, 18, 21, and 24.
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

impl From<Mnemonic> for bip39::Mnemonic {
    fn from(mnemonic: Mnemonic) -> Self {
        mnemonic.inner
    }
}

impl From<&Mnemonic> for bip39::Mnemonic {
    fn from(mnemonic: &Mnemonic) -> Self {
        mnemonic.inner.clone()
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

    /// Return the string representation of the Mnemonic.
    /// This representation can be used to recreate the Mnemonic via `new()`
    ///
    /// Note this is secret information, do not log it.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    /// Creates a Mnemonic from entropy, at least 16 bytes are needed.
    #[wasm_bindgen(js_name = fromEntropy)]
    pub fn from_entropy(b: &[u8]) -> Result<Mnemonic, Error> {
        let inner = bip39::Mnemonic::from_entropy(b)?;
        Ok(inner.into())
    }

    /// Creates a random Mnemonic of given words (12,15,18,21,24)
    #[wasm_bindgen(js_name = fromRandom)]
    pub fn from_random(word_count: usize) -> Result<Mnemonic, Error> {
        let inner = bip39::Mnemonic::generate(word_count)?;
        Ok(inner.into())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
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

        let mnemonic_entropy = Mnemonic::from_entropy(&[1u8; 32]).unwrap();
        assert_eq!(mnemonic_entropy.to_string(), "absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice cage absurd amount doctor acoustic avoid letter advice comic");
        let mnemonic_entropy = Mnemonic::from_entropy(&[1u8; 16]).unwrap();
        assert_eq!(
            mnemonic_entropy.to_string(),
            "absurd amount doctor acoustic avoid letter advice cage absurd amount doctor adjust"
        );

        let err = Mnemonic::from_entropy(&[1u8; 15]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "entropy was not between 128-256 bits or not a multiple of 32 bits: 120 bits"
        );

        let mnemonic_random = Mnemonic::from_random(12).unwrap();
        assert_eq!(mnemonic_random.to_string().split(' ').count(), 12);
        let err = Mnemonic::from_random(11).unwrap_err();
        assert_eq!(
            err.to_string(),
            "mnemonic has an invalid word count: 11. Word count must be 12, 15, 18, 21, or 24"
        );
    }
}
