//! Taproot control block for script-path spending

use crate::{LwkError, XOnlyPublicKey};
use std::sync::Arc;

/// A control block for Taproot script-path spending.
///
/// See [`elements::bitcoin::taproot::ControlBlock`] for more details.
#[derive(uniffi::Object, Debug, Clone)]
pub struct ControlBlock {
    inner: elements::bitcoin::taproot::ControlBlock,
}

impl From<elements::bitcoin::taproot::ControlBlock> for ControlBlock {
    fn from(inner: elements::bitcoin::taproot::ControlBlock) -> Self {
        Self { inner }
    }
}

impl TryFrom<elements::taproot::ControlBlock> for ControlBlock {
    type Error = elements::bitcoin::taproot::TaprootError;

    fn try_from(value: elements::taproot::ControlBlock) -> Result<Self, Self::Error> {
        let inner = elements::bitcoin::taproot::ControlBlock::decode(value.serialize().as_ref())?;
        Ok(Self { inner })
    }
}

impl AsRef<elements::bitcoin::taproot::ControlBlock> for ControlBlock {
    fn as_ref(&self) -> &elements::bitcoin::taproot::ControlBlock {
        &self.inner
    }
}

#[uniffi::export]
impl ControlBlock {
    /// Parse a control block from serialized bytes.
    #[uniffi::constructor]
    pub fn from_slice(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner = elements::bitcoin::taproot::ControlBlock::decode(bytes)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Serialize the control block to bytes.
    pub fn serialize(&self) -> Vec<u8> {
        self.inner.serialize()
    }

    /// Get the leaf version of the control block.
    pub fn leaf_version(&self) -> u8 {
        self.inner.leaf_version.to_consensus()
    }

    /// Get the internal key of the control block.
    pub fn internal_key(&self) -> Arc<XOnlyPublicKey> {
        Arc::new(self.inner.internal_key.into())
    }

    /// Get the output key parity (0 for even, 1 for odd).
    pub fn output_key_parity(&self) -> u8 {
        self.inner.output_key_parity.to_u8()
    }

    /// Get the size of the control block in bytes.
    pub fn size(&self) -> u32 {
        self.inner.size() as u32
    }
}
