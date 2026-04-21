use crate::{
    Address, AssetId, Balance, Error, OutPoint, Script, Transaction, TxOutSecrets, Txid, Wollet,
};
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
    #[wasm_bindgen(js_name = txType)]
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

    /// Unblinded URL
    #[wasm_bindgen(js_name = unblindedUrl)]
    pub fn unblinded_url(&self, explorer_url: &str) -> String {
        self.inner.unblinded_url(explorer_url)
    }

    /// Inputs
    pub fn inputs(&self) -> Vec<TxOutDetails> {
        self.inner.inputs().iter().map(Into::into).collect()
    }

    /// Outputs
    pub fn outputs(&self) -> Vec<TxOutDetails> {
        self.inner.outputs().iter().map(Into::into).collect()
    }

    // TODO: expose fees (need to handle hashmap, do we want to expose it?)
}

/// Transaction output details
#[derive(Debug)]
#[wasm_bindgen]
pub struct TxOutDetails {
    inner: lwk_wollet::TxOutDetails,
}

impl From<&lwk_wollet::TxOutDetails> for TxOutDetails {
    fn from(inner: &lwk_wollet::TxOutDetails) -> Self {
        Self {
            inner: inner.clone(),
        }
    }
}

#[wasm_bindgen]
impl TxOutDetails {
    /// Outpoint
    pub fn outpoint(&self) -> OutPoint {
        self.inner.outpoint().into()
    }

    /// Scriptpubkey
    pub fn script_pubkey(&self) -> Option<Script> {
        self.inner.script_pubkey().map(Into::into)
    }

    /// Height
    pub fn height(&self) -> Option<u32> {
        self.inner.height()
    }

    /// Address
    pub fn address(&self) -> Option<Address> {
        self.inner.address().map(Into::into)
    }

    /// Unblinded values (asset, amount, blinders)
    pub fn unblinded(&self) -> Option<TxOutSecrets> {
        self.inner.unblinded().map(Into::into)
    }

    /// Whether the transaction output is explicit
    pub fn is_explicit(&self) -> bool {
        self.inner.is_explicit()
    }

    /// Whether the output is spent by a previously downloaded transaction
    ///
    /// Note: this value might be inaccurate. We compute this from downloaded
    /// transactions, however we only download transactions relevant for the
    /// wallet (i.e. if they include inputs or outputs that belong to the
    /// wallet), thus for non-wallet outputs we might set this value
    /// incorrectly. For wallet outputs, it can be outdated.
    pub fn is_spent(&self) -> bool {
        self.inner.is_spent()
    }
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

/// Options for transaction details
#[derive(Debug)]
#[wasm_bindgen]
pub struct TxOpt {
    inner: lwk_wollet::TxOpt,
}

#[wasm_bindgen]
impl TxOpt {
    pub fn default() -> Self {
        let inner = lwk_wollet::TxOpt::default();
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

    /// Number of transactions
    #[wasm_bindgen(js_name = numTxs)]
    pub fn num_txs(&self) -> Result<usize, Error> {
        let opt = lwk_wollet::TxsOpt {
            without_tx: true,
            ..Default::default()
        };
        Ok(self.inner().txs(&opt)?.len())
    }

    /// Get the details of a transaction
    ///
    /// **Unstable**: This API may change without notice.
    #[wasm_bindgen(js_name = txDetails)]
    pub fn tx_details(&self, txid: &Txid, opt: &TxOpt) -> Result<Option<TxDetails>, Error> {
        Ok(self
            .inner()
            .tx_details(&txid.into(), &opt.inner)?
            .map(Into::into))
    }
}
