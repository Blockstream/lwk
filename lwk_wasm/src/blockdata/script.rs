use crate::Error;
use lwk_wollet::elements::{self, hex::ToHex, pset::serialize::Deserialize};
use lwk_wollet::hashes::hex::FromHex;
use wasm_bindgen::prelude::*;

/// An Elements (Liquid) script
#[wasm_bindgen]
pub struct Script {
    inner: elements::Script,
}

impl From<elements::Script> for Script {
    fn from(inner: elements::Script) -> Self {
        Self { inner }
    }
}

impl From<&elements::Script> for Script {
    fn from(inner: &elements::Script) -> Self {
        Self {
            inner: inner.clone(),
        }
    }
}

impl AsRef<elements::Script> for Script {
    fn as_ref(&self) -> &elements::Script {
        &self.inner
    }
}

impl std::fmt::Display for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.to_hex())
    }
}

#[wasm_bindgen]
impl Script {
    /// Creates a `Script` from its hex string representation.
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Result<Script, Error> {
        let bytes = Vec::<u8>::from_hex(s)?;
        let inner = elements::Script::deserialize(&bytes[..])?;
        Ok(inner.into())
    }

    /// Creates an empty `Script`.
    pub fn empty() -> Script {
        Script {
            inner: elements::Script::new(),
        }
    }

    /// Return the consensus encoded bytes of the script.
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.as_bytes().to_vec()
    }

    /// Return the string of the script showing op codes and their arguments.
    ///
    /// For example: "OP_DUP OP_HASH160 OP_PUSHBYTES_20 088ac47276d105b91cf9aa27a00112421dd5f23c OP_EQUALVERIFY OP_CHECKSIG"
    pub fn asm(&self) -> String {
        self.inner.asm()
    }

    /// Return the string representation of the script (hex encoding of its consensus encoded bytes).
    /// This representation can be used to recreate the script via `new()`
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use super::Script;
    use lwk_wollet::elements::hex::FromHex;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn script() {
        let script_str = "76a914088ac47276d105b91cf9aa27a00112421dd5f23c88ac";

        let script = Script::new(script_str).unwrap();
        assert_eq!(script.to_string(), script_str);

        let script_bytes = Vec::<u8>::from_hex(script_str).unwrap();
        assert_eq!(script.bytes(), script_bytes);

        assert_eq!(
            script.asm(),
            "OP_DUP OP_HASH160 OP_PUSHBYTES_20 088ac47276d105b91cf9aa27a00112421dd5f23c OP_EQUALVERIFY OP_CHECKSIG"
        );
    }
}
