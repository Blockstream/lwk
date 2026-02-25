use crate::Cmr;

use std::sync::Arc;

use lwk_simplicity::simplicityhl;

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
    pub fn program_bytes(&self) -> Vec<u8> {
        self.pruned.to_vec_with_witness().0
    }

    /// Get the serialized witness bytes.
    pub fn witness_bytes(&self) -> Vec<u8> {
        self.pruned.to_vec_with_witness().1
    }

    /// Get the CMR (Commitment Merkle Root) of the pruned program.
    pub fn cmr(&self) -> Arc<Cmr> {
        Arc::new(self.pruned.cmr().into())
    }

    /// Get the resulting value as a string representation.
    pub fn value(&self) -> String {
        format!("{:?}", self.value)
    }
}
