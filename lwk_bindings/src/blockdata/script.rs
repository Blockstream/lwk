use elements::{hex::ToHex, pset::serialize::Deserialize};

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

    /// Return the consensus encoded bytes of the script.
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.as_bytes().to_vec()
    }

    /// Return the string representation of the script showing op codes and their arguments.
    /// For example: "OP_0 OP_PUSHBYTES_32 d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde"
    pub fn asm(&self) -> String {
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
            script.asm(),
            "OP_0 OP_PUSHBYTES_32 d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde"
        );

        assert!(is_provably_segwit(&script, &None));

        let burn = Script::new(&"6a".parse().unwrap()).unwrap();
        assert!(burn.is_provably_unspendable());
    }
}
