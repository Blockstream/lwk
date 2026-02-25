//! Liquid script

use elements::{
    hashes::{sha256, Hash},
    hex::ToHex,
    pset::serialize::Deserialize,
};

use crate::{types::Hex, LwkError};
use std::{fmt::Display, sync::Arc};

/// A Liquid script
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct Script {
    inner: elements::Script,
}

impl From<elements::Script> for Script {
    fn from(inner: elements::Script) -> Self {
        Self { inner }
    }
}

impl From<Script> for elements::Script {
    fn from(script: Script) -> elements::Script {
        script.inner
    }
}

impl From<&Script> for elements::Script {
    fn from(script: &Script) -> elements::Script {
        script.inner.clone()
    }
}

impl Display for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.to_hex())
    }
}

#[uniffi::export]
impl Script {
    /// Construct a Script object from its hex representation.
    /// To create the hex representation of a script use `to_string()`.
    #[uniffi::constructor]
    pub fn new(hex: &Hex) -> Result<Arc<Self>, LwkError> {
        let inner = elements::Script::deserialize(hex.as_ref())?;
        Ok(Arc::new(Self { inner }))
    }

    /// Create an empty script (for fee outputs).
    #[uniffi::constructor]
    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            inner: elements::Script::new(),
        })
    }

    /// Create an OP_RETURN script (for burn outputs).
    #[uniffi::constructor]
    pub fn new_op_return(data: &[u8]) -> Arc<Self> {
        Arc::new(Self {
            inner: elements::Script::new_op_return(data),
        })
    }

    /// Return the consensus encoded bytes of the script.
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.as_bytes().to_vec()
    }

    /// Returns SHA256 of the script's consensus bytes.
    ///
    /// Returns an equivalent value to the `jet::input_script_hash(index)`/`jet::output_script_hash(index)`.
    pub fn jet_sha256_hex(&self) -> Hex {
        Hex::from(
            sha256::Hash::hash(self.inner.as_bytes())
                .to_byte_array()
                .to_vec(),
        )
    }

    // "asm" is a reserved keyword in some target languages, do not use it
    /// Return the string representation of the script showing op codes and their arguments.
    /// For example: "OP_0 OP_PUSHBYTES_32 d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde"
    pub fn to_asm(&self) -> String {
        self.inner.asm()
    }

    /// Whether a script pubkey is provably unspendable (like a burn script)
    pub fn is_provably_unspendable(&self) -> bool {
        self.inner.is_provably_unspendable()
    }
}

/// Whether a script pubkey is provably segwit
#[uniffi::export]
pub fn is_provably_segwit(script_pubkey: &Script, redeem_script: &Option<Arc<Script>>) -> bool {
    lwk_common::is_provably_segwit(
        &script_pubkey.into(),
        &redeem_script.as_ref().map(|s| s.as_ref().into()),
    )
}

#[cfg(test)]
mod tests {
    use elements::hashes::hex::FromHex;

    use super::{is_provably_segwit, Script};

    #[test]
    fn script() {
        let script_str = "0020d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde";

        let script = Script::new(&script_str.parse().unwrap()).unwrap();
        assert_eq!(script.to_string(), script_str);

        let script_bytes = Vec::<u8>::from_hex(script_str).unwrap();
        assert_eq!(script.bytes(), script_bytes);

        assert_eq!(
            script.to_asm(),
            "OP_0 OP_PUSHBYTES_32 d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde"
        );

        assert!(is_provably_segwit(&script, &None));

        let burn = Script::new(&"6a".parse().unwrap()).unwrap();
        assert!(burn.is_provably_unspendable());
    }

    #[test]
    fn test_script_empty() {
        let script = Script::empty();
        assert!(script.bytes().is_empty());
        assert_eq!(script.to_string(), "");
        assert_eq!(script.to_asm(), "");
    }

    #[test]
    fn test_script_op_return() {
        let data = b"burn".to_vec();
        let script = Script::new_op_return(&data);
        // OP_RETURN + OP_PUSHBYTES_4 + "burn"
        assert!(script.is_provably_unspendable());
        assert!(script.to_asm().starts_with("OP_RETURN"));
    }

    #[test]
    fn test_jet_sha256_hex() {
        let script_str = "51200e3a715a8791642277e1fcb823d974dfb4d8c774ad86deea13a0ba3b2d5ca4d2";
        let expected_hash = "0ea9adeb75ca64bbda18269a25cb94ef71d76627b77243e506e55e9e2962134d";

        let script = Script::new(&script_str.parse().unwrap()).unwrap();
        assert_eq!(script.jet_sha256_hex().to_string(), expected_hash);
    }
}
