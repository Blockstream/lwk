//! External UTXO

use std::sync::Arc;

use crate::{LwkError, OutPoint, Transaction, TxOut, TxOutSecrets};

/// An external UTXO, owned by another wallet
#[derive(uniffi::Object)]
pub struct ExternalUtxo {
    inner: lwk_wollet::ExternalUtxo,
}

#[uniffi::export]
impl ExternalUtxo {
    /// Construct an ExternalUtxo
    #[uniffi::constructor]
    pub fn new(
        vout: u32,
        tx: &Transaction,
        unblinded: &TxOutSecrets,
        max_weight_to_satisfy: u32,
        is_segwit: bool,
    ) -> Result<Arc<Self>, LwkError> {
        let tx: elements::Transaction = tx.into();
        let txout = tx
            .output
            .get(vout as usize)
            .ok_or_else(|| LwkError::Generic {
                msg: "Transaction does not have enough outputs".to_string(),
            })?
            .clone();
        let outpoint = elements::OutPoint::new(tx.txid(), vout);
        let tx = if is_segwit { None } else { Some(tx) };
        let utxo = lwk_wollet::ExternalUtxo {
            outpoint,
            txout,
            tx,
            unblinded: unblinded.into(),
            max_weight_to_satisfy: max_weight_to_satisfy as usize,
        };
        Ok(Arc::new(utxo.into()))
    }

    /// Construct an `ExternalUtxo` from unchecked outpoint/txout data.
    ///
    /// Unlike [`ExternalUtxo::new`], this constructor does not inspect a parent transaction or
    /// derive `outpoint` and `txout` from it. It exists as an optimisation for callers that
    /// already hold the exact outpoint, prevout, and unblinded data and want to avoid fetching or
    /// materialising the full transaction just to construct the bindings object.
    ///
    /// Do not use this for pre-segwit external UTXOs. Pre-segwit inputs require the parent
    /// transaction so the builder can populate `non_witness_utxo`, while this constructor always
    /// creates an `ExternalUtxo` without it.
    ///
    /// Use this cautiously. Callers are responsible for ensuring that `outpoint` and `txout`
    /// describe the same UTXO. As with [`ExternalUtxo::new`], callers must also ensure that
    /// `unblinded` and `max_weight_to_satisfy` match that UTXO.
    ///
    /// IMPORTANT: This is a temporary workaround to speed up integration work
    /// and should be removed after a more complete migration to `lwk`.
    // TODO: this is a temporary solution and is meant to speed up the integration work;
    // it should be removed after a more complete migration to lwk
    #[uniffi::constructor]
    pub fn from_unchecked_data(
        outpoint: &OutPoint,
        txout: &TxOut,
        unblinded: &TxOutSecrets,
        max_weight_to_satisfy: u32,
    ) -> Arc<Self> {
        Arc::new(
            lwk_wollet::ExternalUtxo {
                outpoint: outpoint.into(),
                txout: txout.into(),
                tx: None,
                unblinded: unblinded.into(),
                max_weight_to_satisfy: max_weight_to_satisfy as usize,
            }
            .into(),
        )
    }
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

// TODO: method to inspect inner
