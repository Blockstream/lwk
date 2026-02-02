//! External UTXO

use crate::{Error, Transaction, TxOutSecrets};
use wasm_bindgen::prelude::*;

/// An external UTXO, owned by another wallet.
#[wasm_bindgen]
pub struct ExternalUtxo {
    inner: lwk_wollet::ExternalUtxo,
}

impl From<lwk_wollet::ExternalUtxo> for ExternalUtxo {
    fn from(inner: lwk_wollet::ExternalUtxo) -> Self {
        Self { inner }
    }
}

impl From<&ExternalUtxo> for lwk_wollet::ExternalUtxo {
    fn from(value: &ExternalUtxo) -> Self {
        value.inner.clone()
    }
}

#[wasm_bindgen]
impl ExternalUtxo {
    /// Construct an ExternalUtxo
    #[wasm_bindgen(constructor)]
    pub fn new(
        vout: u32,
        tx: &Transaction,
        unblinded: &TxOutSecrets,
        max_weight_to_satisfy: u32,
        is_segwit: bool,
    ) -> Result<ExternalUtxo, Error> {
        let tx: lwk_wollet::elements::Transaction = tx.as_ref().clone();
        let txout = tx
            .output
            .get(vout as usize)
            .ok_or_else(|| Error::Generic("Transaction does not have enough outputs".to_string()))?
            .clone();
        let outpoint = lwk_wollet::elements::OutPoint::new(tx.txid(), vout);
        let tx = if is_segwit { None } else { Some(tx) };
        let utxo = lwk_wollet::ExternalUtxo {
            outpoint,
            txout,
            tx,
            unblinded: unblinded.into(),
            max_weight_to_satisfy: max_weight_to_satisfy as usize,
        };
        Ok(utxo.into())
    }
}
