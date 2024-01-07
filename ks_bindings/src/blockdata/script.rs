use elements::{
    hex::ToHex,
    pset::serialize::{Deserialize, Serialize},
};

use crate::{types::Hex, Error};
use std::{fmt::Display, sync::Arc};

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

impl Display for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.serialize().to_hex())
    }
}

#[uniffi::export]
impl Script {
    /// Construct a Script object
    #[uniffi::constructor]
    pub fn new(hex: Hex) -> Result<Arc<Self>, Error> {
        let inner = elements::Script::deserialize(hex.as_ref())?;
        Ok(Arc::new(Self { inner }))
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.inner.as_bytes().to_vec()
    }

    pub fn asm(&self) -> String {
        self.inner.asm()
    }
}

#[cfg(test)]
mod tests {
    use elements::hashes::hex::FromHex;

    use super::Script;

    #[test]
    fn script() {
        let script_str = "0020d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde";

        let script = Script::new(script_str.parse().unwrap()).unwrap();
        assert_eq!(script.to_string(), script_str);

        let script_bytes = Vec::<u8>::from_hex(script_str).unwrap();
        assert_eq!(script.bytes(), script_bytes);

        assert_eq!(
            script.asm(),
            "OP_0 OP_PUSHBYTES_32 d2e99f0c38089c08e5e1080ff6658c6075afaa7699d384333d956c470881afde"
        );
    }
}
