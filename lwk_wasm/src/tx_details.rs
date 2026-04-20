use crate::{AssetId, Balance, Error, Transaction, Txid, Wollet};
use wasm_bindgen::prelude::*;

/// Transaction details
#[derive(Debug)]
#[wasm_bindgen]
pub struct TxDetails {
    inner: lwk_wollet::TxDetails,
}

impl From<lwk_wollet::TxDetails> for TxDetails {
    fn from(inner: lwk_wollet::TxDetails) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl TxDetails {
    /// Transaction
    pub fn tx(&self) -> Option<Transaction> {
        self.inner.tx().cloned().map(Into::into)
    }

    /// Txid
    pub fn txid(&self) -> Txid {
        self.inner.txid().into()
    }

    /// Blockchain height
    pub fn height(&self) -> Option<u32> {
        self.inner.height()
    }

    /// Timestamp
    ///
    /// A reasonable timestamp, that however can be inaccurate.
    /// If you need a precise timestamp, do not use this value.
    pub fn timestamp(&self) -> Option<u32> {
        self.inner.timestamp()
    }

    /// Transaction type
    ///
    /// A tentative description of the transaction type, which
    /// however might be inaccurate. Use this if you want a simple
    /// description of what this transaction is doing, but do
    /// not rely on the value returned.
    pub fn tx_type(&self) -> String {
        self.inner.tx_type().into()
    }

    /// Balance
    ///
    /// Net balance from the `Wollet` perspective
    pub fn balance(&self) -> Balance {
        self.inner.balance().clone().into()
    }

    /// Asset fees
    pub fn fees_asset(&self, asset: &AssetId) -> u64 {
        self.inner.fees_asset(&asset.into())
    }

    // TODO: expose fees (need to handle hashmap, do we want to expose it?)
    // TODO: expose inputs (needs TxOutDetails wrapping)
    // TODO: expose outputs (needs TxOutDetails wrapping)
    // TODO: expose unblinded_url (needs lwk_wollet::TxDetails::unblinded_url())
}

/// Options for transaction details
#[derive(Debug)]
#[wasm_bindgen]
pub struct TxsOpt {
    inner: lwk_wollet::TxsOpt,
}

#[wasm_bindgen]
impl TxsOpt {
    pub fn default() -> Self {
        let inner = lwk_wollet::TxsOpt::default();
        Self { inner }
    }

    #[wasm_bindgen(js_name = withPagination)]
    pub fn with_pagination(offset: usize, limit: usize) -> Self {
        let inner = lwk_wollet::TxsOpt {
            offset: Some(offset),
            limit: Some(limit),
            ..Default::default()
        };
        Self { inner }
    }
}

#[wasm_bindgen]
impl Wollet {
    /// Get the transaction list
    ///
    /// **Unstable**: This API may change without notice.
    pub fn txs(&self, opt: &TxsOpt) -> Result<Vec<TxDetails>, Error> {
        Ok(self
            .inner()
            .txs(&opt.inner)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    // TODO: tx_details
}
