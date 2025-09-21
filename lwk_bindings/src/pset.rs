use crate::{types::AssetId, Issuance, LwkError, Script, Transaction, Txid};
use elements::pset::{Input, PartiallySignedTransaction};
use elements::{hashes::Hash, BlockHash};
use lwk_wollet::elements_miniscript::psbt::finalize;
use lwk_wollet::EC;
use std::{fmt::Display, sync::Arc};

/// A Partially Signed Elements Transaction
#[derive(uniffi::Object, PartialEq, Debug, Clone)]
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

impl AsRef<PartiallySignedTransaction> for Pset {
    fn as_ref(&self) -> &PartiallySignedTransaction {
        &self.inner
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

    /// Finalize and extract the PSET
    pub fn finalize(&self) -> Result<Arc<Transaction>, LwkError> {
        let mut pset = self.inner.clone();
        finalize(&mut pset, &EC, BlockHash::all_zeros())?;
        let tx: Transaction = pset.extract_tx()?.into();
        Ok(Arc::new(tx))
    }

    /// Extract the Transaction from a Pset by filling in
    /// the available signature information in place.
    pub fn extract_tx(&self) -> Result<Arc<Transaction>, LwkError> {
        let tx: Transaction = self.inner.extract_tx()?.into();
        Ok(Arc::new(tx))
    }

    /// Attempt to combine with another `Pset`.
    pub fn combine(&self, other: &Pset) -> Result<Pset, LwkError> {
        let mut pset = self.inner.clone();
        pset.merge(other.inner.clone())?;
        Ok(pset.into())
    }

    /// Get the unique id of the PSET as defined by [BIP-370](https://github.com/bitcoin/bips/blob/master/bip-0370.mediawiki#unique-identification)
    ///
    /// The unique id is the txid of the PSET with sequence numbers of inputs set to 0
    pub fn unique_id(&self) -> Result<Txid, LwkError> {
        let txid = self.inner.unique_id()?;
        Ok(txid.into())
    }

    /// Return a copy of the inputs of this PSET
    pub fn inputs(&self) -> Vec<Arc<PsetInput>> {
        self.inner
            .inputs()
            .iter()
            .map(|i| Arc::new(i.clone().into()))
            .collect()
    }
}

impl Pset {
    pub(crate) fn inner(&self) -> PartiallySignedTransaction {
        self.inner.clone()
    }
}

/// PSET input
#[derive(uniffi::Object, Debug)]
pub struct PsetInput {
    inner: Input,
}

impl From<Input> for PsetInput {
    fn from(inner: Input) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl PsetInput {
    /// Prevout TXID of the input
    pub fn previous_txid(&self) -> Arc<Txid> {
        Arc::new(self.inner.previous_txid.into())
    }

    /// Prevout vout of the input
    pub fn previous_vout(&self) -> u32 {
        self.inner.previous_output_index
    }

    /// Prevout scriptpubkey of the input
    pub fn previous_script_pubkey(&self) -> Option<Arc<Script>> {
        self.inner
            .witness_utxo
            .as_ref()
            .map(|txout| Arc::new(txout.script_pubkey.clone().into()))
    }

    /// Redeem script of the input
    pub fn redeem_script(&self) -> Option<Arc<Script>> {
        self.inner
            .redeem_script
            .as_ref()
            .map(|s| Arc::new(s.clone().into()))
    }

    /// If the input has an issuance, the asset id
    pub fn issuance_asset(&self) -> Option<AssetId> {
        self.inner
            .has_issuance()
            .then(|| self.inner.issuance_ids().0.into())
    }

    /// If the input has an issuance, the token id
    pub fn issuance_token(&self) -> Option<AssetId> {
        self.inner
            .has_issuance()
            .then(|| self.inner.issuance_ids().1.into())
    }

    /// If the input has a (re)issuance, the issuance object
    pub fn issuance(&self) -> Option<Arc<Issuance>> {
        self.inner
            .has_issuance()
            .then(|| Arc::new(lwk_common::Issuance::new(&self.inner).into()))
    }

    /// Input sighash
    pub fn sighash(&self) -> u32 {
        self.inner.sighash_type.map(|s| s.to_u32()).unwrap_or(1)
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
        let tx = pset.extract_tx().unwrap();
        assert_eq!(tx_expected, tx.to_string());

        assert_eq!(pset_string, pset.to_string());

        assert_eq!(pset.inputs().len(), tx.inputs().len());
        let pset_in = &pset.inputs()[0];
        let tx_in = &tx.inputs()[0];
        assert_eq!(pset_in.previous_txid(), tx_in.outpoint().txid());
        assert_eq!(pset_in.previous_vout(), tx_in.outpoint().vout());
        assert!(pset_in.previous_script_pubkey().is_some());
        assert!(pset_in.redeem_script().is_none());

        assert!(pset_in.issuance_asset().is_none());
        assert!(pset_in.issuance_token().is_none());
    }

    #[test]
    fn pset_combine() {
        let psets = lwk_test_util::psets_to_combine().1;
        let psets: Vec<Pset> = psets.into_iter().map(Into::into).collect();
        psets[0].finalize().unwrap_err(); // not enough signatures

        let pset01 = psets[0].combine(&psets[1]).unwrap();
        pset01.finalize().unwrap_err(); // not enough signatures
        let pset012 = pset01.combine(&psets[2]).unwrap();
        pset012.finalize().unwrap(); // enough signatures
    }

    #[test]
    fn pset_unique_id() {
        let psets = lwk_test_util::psets_to_combine().1;
        let psets: Vec<Pset> = psets.into_iter().map(Into::into).collect();

        let unique_id = psets[0].unique_id().unwrap();
        for pset in psets.iter().skip(1) {
            assert_eq!(unique_id, pset.unique_id().unwrap());

            // sequence number is 0xffffffff, unique id set it to 0 before computing the hash, so txid is different
            assert_ne!(unique_id, *pset.extract_tx().unwrap().txid());
        }
    }
}
