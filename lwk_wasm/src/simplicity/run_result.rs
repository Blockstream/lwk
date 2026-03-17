use super::cmr::Cmr;

use std::sync::Arc;

use lwk_simplicity::simplicityhl::simplicity::{jet, RedeemNode, Value};

use wasm_bindgen::prelude::*;

/// The result of running a Simplicity program.
#[wasm_bindgen]
pub struct SimplicityRunResult {
    pub(crate) pruned: Arc<RedeemNode<jet::Elements>>,
    pub(crate) value: Value,
}

#[wasm_bindgen]
impl SimplicityRunResult {
    /// Get the serialized program bytes.
    #[wasm_bindgen(getter = programBytes)]
    pub fn program_bytes(&self) -> Vec<u8> {
        let (program_bytes, _) = self.pruned.to_vec_with_witness();
        program_bytes
    }

    /// Get the serialized witness bytes.
    #[wasm_bindgen(getter = witnessBytes)]
    pub fn witness_bytes(&self) -> Vec<u8> {
        let (_, witness_bytes) = self.pruned.to_vec_with_witness();
        witness_bytes
    }

    /// Get the Commitment Merkle Root of the pruned program.
    #[wasm_bindgen(getter = cmr)]
    pub fn cmr(&self) -> Cmr {
        self.pruned.cmr().into()
    }

    /// Get the resulting value as a string representation.
    #[wasm_bindgen(getter = value)]
    pub fn value(&self) -> String {
        format!("{:?}", self.value)
    }
}
