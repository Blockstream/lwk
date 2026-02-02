use crate::{OutPoint, TxSequence};

use super::tx_in_witness::TxInWitness;

use lwk_wollet::elements;

use wasm_bindgen::prelude::*;

/// A transaction input.
#[wasm_bindgen]
pub struct TxIn {
    inner: elements::TxIn,
}

impl From<elements::TxIn> for TxIn {
    fn from(inner: elements::TxIn) -> Self {
        Self { inner }
    }
}

impl From<TxIn> for elements::TxIn {
    fn from(value: TxIn) -> Self {
        value.inner
    }
}

impl AsRef<elements::TxIn> for TxIn {
    fn as_ref(&self) -> &elements::TxIn {
        &self.inner
    }
}

#[wasm_bindgen]
impl TxIn {
    /// Get the outpoint (previous output) for this input.
    pub fn outpoint(&self) -> OutPoint {
        self.inner.previous_output.into()
    }

    /// Get the witness for this input.
    pub fn witness(&self) -> TxInWitness {
        self.inner.witness.clone().into()
    }

    /// Get the sequence number for this input.
    pub fn sequence(&self) -> TxSequence {
        self.inner.sequence.into()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::TxIn;
    use lwk_wollet::elements;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_tx_in() {
        let txid = elements::Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let outpoint = elements::OutPoint::new(txid, 1);
        let elements_tx_in = elements::TxIn {
            previous_output: outpoint,
            is_pegin: false,
            script_sig: elements::Script::new(),
            sequence: elements::Sequence::MAX,
            asset_issuance: elements::AssetIssuance::default(),
            witness: elements::TxInWitness::default(),
        };

        let tx_in: TxIn = elements_tx_in.clone().into();
        assert_eq!(tx_in.outpoint().vout(), 1);
        assert!(tx_in.sequence().is_final());
        assert!(tx_in.witness().is_empty());
    }
}
