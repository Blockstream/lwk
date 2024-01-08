use elements::{
    hex::ToHex,
    pset::serialize::{Deserialize, Serialize},
};
use wollet::WalletTx;

use crate::{types::Hex, Error, Txid};
use std::{fmt::Display, sync::Arc};

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct Transaction {
    inner: elements::Transaction,
}

impl From<WalletTx> for Transaction {
    fn from(value: WalletTx) -> Self {
        Self { inner: value.tx }
    }
}

impl From<elements::Transaction> for Transaction {
    fn from(inner: elements::Transaction) -> Self {
        Self { inner }
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.serialize().to_hex())
    }
}

#[uniffi::export]
impl Transaction {
    /// Construct a Transaction object
    #[uniffi::constructor]
    pub fn new(hex: Hex) -> Result<Arc<Self>, Error> {
        let inner: elements::Transaction = elements::Transaction::deserialize(hex.as_ref())?;
        Ok(Arc::new(Self { inner }))
    }

    pub fn txid(&self) -> Arc<Txid> {
        Arc::new(self.inner.txid().into())
    }
}
