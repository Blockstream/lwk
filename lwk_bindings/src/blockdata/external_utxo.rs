//! External UTXO

use std::sync::Arc;

use crate::{LwkError, Transaction, TxOutSecrets};

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
