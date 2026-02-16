//! Liquid transaction input

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

impl AsRef<elements::TxIn> for TxIn {
    fn as_ref(&self) -> &elements::TxIn {
        &self.inner
    }
}

#[uniffi::export]
impl TxIn {
    /// Outpoint
    pub fn outpoint(&self) -> Arc<OutPoint> {
        Arc::new(self.inner.previous_output.into())
    }

    /// Get the sequence number for this input.
    pub fn sequence(&self) -> u32 {
        self.inner.sequence.0
    }
}

#[cfg(feature = "simplicity")]
#[uniffi::export]
impl TxIn {
    /// Get the witness for this input.
    pub fn witness(&self) -> Arc<crate::blockdata::tx_in_witness::TxInWitness> {
        Arc::new(self.inner.witness.clone().into())
    }
}
