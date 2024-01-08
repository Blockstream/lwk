use crate::{types::AssetId, Transaction, WalletTxOut};
use std::{collections::HashMap, sync::Arc};

#[derive(uniffi::Object)]
pub struct WalletTx {
    inner: wollet::WalletTx,
}

impl From<wollet::WalletTx> for WalletTx {
    fn from(inner: wollet::WalletTx) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl WalletTx {
    pub fn tx(&self) -> Arc<Transaction> {
        let tx: Transaction = self.inner.tx.clone().into();
        Arc::new(tx)
    }

    pub fn height(&self) -> Option<u32> {
        self.inner.height
    }

    pub fn balance(&self) -> HashMap<AssetId, i64> {
        self.inner
            .balance
            .iter()
            .map(|(k, v)| (AssetId::from(*k), *v))
            .collect()
    }

    pub fn fee(&self) -> u64 {
        self.inner.fee
    }

    pub fn timestamp(&self) -> Option<u32> {
        self.inner.timestamp
    }

    pub fn inputs(&self) -> Vec<Option<Arc<WalletTxOut>>> {
        self.inner
            .inputs
            .iter()
            .map(|e| e.as_ref().cloned().map(Into::into).map(Arc::new))
            .collect()
    }

    pub fn outputs(&self) -> Vec<Option<Arc<WalletTxOut>>> {
        self.inner
            .outputs
            .iter()
            .map(|e| e.as_ref().cloned().map(Into::into).map(Arc::new))
            .collect()
    }
}

// TODO add basic test
