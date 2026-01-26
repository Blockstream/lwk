//! Taproot control block for script-path spending

use crate::{types::Hex, LwkError, XOnlyPublicKey};
use elements::bitcoin::{
    script::ScriptBuf,
    taproot::{LeafVersion, TaprootBuilder},
};
use std::sync::Arc;

const SIMPLICITY_LEAF_VERSION: u8 = 0xbe;

/// A control block for Taproot script-path spending.
#[derive(uniffi::Object, Debug, Clone)]
pub struct ControlBlock {
    inner: elements::bitcoin::taproot::ControlBlock,
}

impl From<elements::bitcoin::taproot::ControlBlock> for ControlBlock {
    fn from(inner: elements::bitcoin::taproot::ControlBlock) -> Self {
        Self { inner }
    }
}

impl AsRef<elements::bitcoin::taproot::ControlBlock> for ControlBlock {
    fn as_ref(&self) -> &elements::bitcoin::taproot::ControlBlock {
        &self.inner
    }
}

#[uniffi::export]
impl ControlBlock {
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
}

/// Create a control block for a single-leaf Simplicity script.
///
/// This constructs a Taproot control block for script-path spending
/// with a Simplicity program. The CMR (Commitment Merkle Root) serves
/// as the leaf script.
#[uniffi::export]
pub fn simplicity_control_block(
    cmr: &Hex,
    internal_key: &XOnlyPublicKey,
) -> Result<Arc<ControlBlock>, LwkError> {
    let cmr_bytes: &[u8] = cmr.as_ref();
    let script: ScriptBuf = cmr_bytes.to_vec().into();
    let leaf_version = LeafVersion::from_consensus(SIMPLICITY_LEAF_VERSION)?;
    let secp = elements::bitcoin::secp256k1::Secp256k1::verification_only();

    let builder = TaprootBuilder::new().add_leaf_with_ver(0, script.clone(), leaf_version)?;
    let spend_info = builder
        .finalize(&secp, *internal_key.as_ref())
        .map_err(|_| LwkError::Generic {
            msg: "Failed to finalize taproot builder".to_string(),
        })?;

    let control_block = spend_info
        .control_block(&(script, leaf_version))
        .ok_or_else(|| LwkError::Generic {
            msg: "Failed to compute control block".to_string(),
        })?;

    Ok(Arc::new(ControlBlock::from(control_block)))
}
