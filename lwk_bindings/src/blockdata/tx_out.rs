use crate::Script;
use std::sync::Arc;

#[derive(uniffi::Object, Debug)]
pub struct TxOut {
    inner: elements::TxOut,
}

impl From<elements::TxOut> for TxOut {
    fn from(inner: elements::TxOut) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl TxOut {
    /// Scriptpubkey
    pub fn script_pubkey(&self) -> Arc<Script> {
        let spk = self.inner.script_pubkey.clone().into();
        Arc::new(spk)
    }

    /// Whether or not this output is a fee output
    pub fn is_fee(&self) -> bool {
        self.inner.is_fee()
    }
}
