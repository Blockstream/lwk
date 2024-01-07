use std::{fmt::Display, sync::Arc};

use crate::Transaction;

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct WalletTx {
    pub(crate) inner: wollet::WalletTx, // TODO: we should instead use rpc_model data?
}

impl From<wollet::WalletTx> for WalletTx {
    fn from(inner: wollet::WalletTx) -> Self {
        Self { inner }
    }
}

impl Display for WalletTx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: remove unwrap, avoid string allocation
        let s = serde_json::to_string(&self.inner).unwrap();
        write!(f, "{}", s)
    }
}

#[uniffi::export]
impl WalletTx {
    pub fn tx(&self) -> Arc<Transaction> {
        let tx: Transaction = self.inner.tx.clone().into();
        Arc::new(tx)
    }
}
