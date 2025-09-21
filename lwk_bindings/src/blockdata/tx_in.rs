use crate::OutPoint;
use std::sync::Arc;

/// A transaction input.
#[derive(uniffi::Object, Debug)]
pub struct TxIn {
    inner: elements::TxIn,
}

impl From<elements::TxIn> for TxIn {
    fn from(inner: elements::TxIn) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl TxIn {
    /// Outpoint
    pub fn outpoint(&self) -> Arc<OutPoint> {
        Arc::new(self.inner.previous_output.into())
    }
}
