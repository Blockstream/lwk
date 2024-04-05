use elements::{
    hex::ToHex,
    pset::serialize::{Deserialize, Serialize},
};
use lwk_wollet::WalletTx;

use crate::{
    types::{AssetId, Hex},
    LwkError, Txid,
};
use std::{fmt::Display, sync::Arc};

#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone)]
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

impl From<Transaction> for elements::Transaction {
    fn from(value: Transaction) -> Self {
        value.inner
    }
}

impl From<&Transaction> for elements::Transaction {
    fn from(value: &Transaction) -> Self {
        value.inner.clone()
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.serialize().to_hex())
    }
}

impl AsRef<elements::Transaction> for Transaction {
    fn as_ref(&self) -> &elements::Transaction {
        &self.inner
    }
}

#[uniffi::export]
impl Transaction {
    /// Construct a Transaction object
    #[uniffi::constructor]
    pub fn new(hex: &Hex) -> Result<Arc<Self>, LwkError> {
        let inner: elements::Transaction = elements::Transaction::deserialize(hex.as_ref())?;
        Ok(Arc::new(Self { inner }))
    }

    pub fn txid(&self) -> Arc<Txid> {
        Arc::new(self.inner.txid().into())
    }

    pub fn bytes(&self) -> Vec<u8> {
        elements::Transaction::serialize(&self.inner)
    }

    pub fn fee(&self, policy_asset: &AssetId) -> u64 {
        self.inner.fee_in((*policy_asset).into())
    }
}

#[cfg(test)]
mod tests {
    use elements::hex::ToHex;

    use super::Transaction;

    #[test]
    fn transaction() {
        let tx_expected =
            include_str!("../../../lwk_jade/test_data/pset_to_be_signed_transaction.hex")
                .to_string();
        let tx = Transaction::new(&tx_expected.parse().unwrap()).unwrap();

        assert_eq!(tx_expected, tx.to_string());

        assert_eq!(
            tx.txid().to_string(),
            "954f32449d00a9de3c42758dedee895c88ea417cb72999738b2631bcc00e13ad"
        );

        assert_eq!(tx.bytes().to_hex(), tx_expected);
    }
}
