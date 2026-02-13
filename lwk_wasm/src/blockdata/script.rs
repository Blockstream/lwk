use crate::Error;
use lwk_wollet::elements::{self, hex::ToHex, pset::serialize::Deserialize};
use lwk_wollet::hashes::{hex::FromHex, sha256, Hash};
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

    /// Returns SHA256 of the script's consensus bytes.
    ///
    /// Returns an equivalent value to the `jet::input_script_hash(index)`/`jet::output_script_hash(index)`.
    pub fn jet_sha256_hex(&self) -> String {
        sha256::Hash::hash(self.inner.as_bytes())
            .to_byte_array()
            .to_hex()
    }

    /// Return the string of the script showing op codes and their arguments.
    ///
    /// For example: "OP_DUP OP_HASH160 OP_PUSHBYTES_20 088ac47276d105b91cf9aa27a00112421dd5f23c OP_EQUALVERIFY OP_CHECKSIG"
    pub fn asm(&self) -> String {
        self.inner.asm()
    }

    /// Creates an OP_RETURN script with the given data.
    #[wasm_bindgen(js_name = newOpReturn)]
    pub fn new_op_return(data: &[u8]) -> Script {
        Script {
            inner: elements::Script::new_op_return(data),
        }
    }

    /// Returns true if the script is provably unspendable.
    ///
    /// A script is provably unspendable if it starts with OP_RETURN or is larger
    /// than the maximum script size.
    #[wasm_bindgen(js_name = isProvablyUnspendable)]
    pub fn is_provably_unspendable(&self) -> bool {
        self.inner.is_provably_unspendable()
    }

    /// Returns true if this script_pubkey is provably SegWit.
    ///
    /// This checks if the script_pubkey is provably SegWit based on the
    /// script_pubkey itself and an optional redeem_script.
    #[wasm_bindgen(js_name = isProvablySegwit)]
    pub fn is_provably_segwit(&self, redeem_script: Option<Script>) -> bool {
        lwk_common::is_provably_segwit(
            &self.inner.clone(),
            &redeem_script.as_ref().map(|s| s.as_ref().clone()),
        )
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
        let script_str = "0020d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde";

        let script = Script::new(script_str).unwrap();
        assert_eq!(script.to_string(), script_str);

        let script_bytes = Vec::<u8>::from_hex(script_str).unwrap();
        assert_eq!(script.bytes(), script_bytes);

        assert_eq!(
            script.asm(),
            "OP_0 OP_PUSHBYTES_32 d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde"
        );

        assert!(script.is_provably_segwit(None));

        assert!(Script::new("6a").unwrap().is_provably_unspendable());
        assert!(Script::empty().bytes().is_empty());
        assert!(Script::new_op_return(b"burn")
            .asm()
            .starts_with("OP_RETURN"));
    }

    #[wasm_bindgen_test]
    fn test_jet_sha256_hex() {
        let script_str = "51200e3a715a8791642277e1fcb823d974dfb4d8c774ad86deea13a0ba3b2d5ca4d2";
        let expected_hash = "0ea9adeb75ca64bbda18269a25cb94ef71d76627b77243e506e55e9e2962134d";

        let script = Script::new(script_str).unwrap();
        assert_eq!(script.jet_sha256_hex(), expected_hash);
    }
}
