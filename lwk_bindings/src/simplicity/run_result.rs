use std::sync::Arc;

use lwk_simplicity_options::simplicityhl;

use crate::types::Hex;

/// The result of running a Simplicity program.
#[derive(uniffi::Object)]
pub struct SimplicityRunResult {
    pub(crate) pruned:
        Arc<simplicityhl::simplicity::RedeemNode<simplicityhl::simplicity::jet::Elements>>,
    pub(crate) value: simplicityhl::simplicity::Value,
}

#[uniffi::export]
impl SimplicityRunResult {
    /// Get the serialized program bytes.
    pub fn program_bytes(&self) -> Hex {
        let (program_bytes, _) = self.pruned.to_vec_with_witness();
        Hex::from(program_bytes)
    }

    /// Get the serialized witness bytes.
    pub fn witness_bytes(&self) -> Hex {
        let (_, witness_bytes) = self.pruned.to_vec_with_witness();
        Hex::from(witness_bytes)
    }

    /// Get the CMR (Commitment Merkle Root) of the pruned program.
    pub fn cmr(&self) -> Hex {
        let cmr = self.pruned.cmr();
        Hex::from(cmr.as_ref().to_vec())
    }

    /// Get the resulting value as a string representation.
    pub fn value(&self) -> String {
        format!("{:?}", self.value)
    }
}
