use crate::wallet_abi::schema::FinalizerSpec;

use std::collections::HashMap;

use lwk_wollet::elements::TxOutSecrets;

pub(crate) struct ResolutionArtifacts {
    secrets: HashMap<usize, TxOutSecrets>,
    finalizers: Vec<FinalizerSpec>,
    wallet_input_indices: Vec<u32>,
    wallet_input_finalization_weight: usize,
}

impl ResolutionArtifacts {
    pub(crate) fn secrets(&self) -> &HashMap<usize, TxOutSecrets> {
        &self.secrets
    }

    pub(crate) fn finalizers(&self) -> &[FinalizerSpec] {
        &self.finalizers
    }

    pub(crate) fn wallet_input_indices(&self) -> &[u32] {
        &self.wallet_input_indices
    }

    pub(crate) fn wallet_input_finalization_weight(&self) -> usize {
        self.wallet_input_finalization_weight
    }
}
