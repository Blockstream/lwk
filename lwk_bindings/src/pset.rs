use crate::{types::AssetId, LwkError, Transaction};
use elements::pset::PartiallySignedTransaction;
use std::{fmt::Display, sync::Arc};

/// Partially Signed Elements Transaction, wrapper over [`elements::pset::PartiallySignedTransaction`]
#[derive(uniffi::Object, PartialEq, Debug)]
#[uniffi::export(Display)]
pub struct Pset {
    inner: PartiallySignedTransaction,
}

impl From<PartiallySignedTransaction> for Pset {
    fn from(inner: PartiallySignedTransaction) -> Self {
        Self { inner }
    }
}

impl Display for Pset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl Pset {
    /// Construct a Watch-Only wallet object
    #[uniffi::constructor]
    pub fn new(base64: &str) -> Result<Arc<Self>, LwkError> {
        let inner: PartiallySignedTransaction = base64.trim().parse()?;
        Ok(Arc::new(Pset { inner }))
    }

    pub fn extract_tx(&self) -> Result<Arc<Transaction>, LwkError> {
        let tx: Transaction = self.inner.extract_tx()?.into();
        Ok(Arc::new(tx))
    }

    pub fn issuance_asset(&self, index: u32) -> Option<AssetId> {
        self.issuances_ids(index).map(|e| e.0)
    }

    pub fn issuance_token(&self, index: u32) -> Option<AssetId> {
        self.issuances_ids(index).map(|e| e.1)
    }
}

impl Pset {
    fn issuances_ids(&self, index: u32) -> Option<(AssetId, AssetId)> {
        let issuance_ids = self.inner.inputs().get(index as usize)?.issuance_ids();
        Some((issuance_ids.0.into(), issuance_ids.1.into()))
    }

    pub(crate) fn inner(&self) -> PartiallySignedTransaction {
        self.inner.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::Pset;

    #[test]
    fn pset_roundtrip() {
        let pset_string =
            include_str!("../../lwk_jade/test_data/pset_to_be_signed.base64").to_string();
        let pset = Pset::new(&pset_string).unwrap();

        let tx_expected =
            include_str!("../../lwk_jade/test_data/pset_to_be_signed_transaction.hex").to_string();
        let tx_string = pset.extract_tx().unwrap().to_string();
        assert_eq!(tx_expected, tx_string);

        assert_eq!(pset_string, pset.to_string());
    }
}
