mod input;
mod output;

pub use input::{PsetInput, PsetInputBuilder};
pub use output::{PsetOutput, PsetOutputBuilder};

use crate::{LwkError, Transaction, Txid};

use std::fmt::Display;
use std::sync::Arc;

use elements::pset::PartiallySignedTransaction;
use elements::{hashes::Hash, BlockHash};

use lwk_wollet::elements_miniscript::psbt::finalize;
use lwk_wollet::EC;

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
            .map(|i| Arc::new(PsetInput::from_inner(i.clone())))
            .collect()
    }
}

impl Pset {
    pub(crate) fn inner(&self) -> PartiallySignedTransaction {
        self.inner.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::Pset;
    use crate::{OutPoint, Script, TxSequence, Txid};

    #[test]
    fn pset_roundtrip() {
        let pset_string =
            include_str!("../../../lwk_jade/test_data/pset_to_be_signed.base64").to_string();
        let pset = Pset::new(&pset_string).unwrap();

        let tx_expected =
            include_str!("../../../lwk_jade/test_data/pset_to_be_signed_transaction.hex")
                .to_string();
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

    #[test]
    fn pset_input_builder() {
        use super::PsetInputBuilder;

        let txid = Txid::new(
            &"0000000000000000000000000000000000000000000000000000000000000001"
                .parse()
                .unwrap(),
        )
        .unwrap();
        let outpoint = OutPoint::from_parts(&txid, 0);
        let builder = PsetInputBuilder::from_prevout(&outpoint);
        builder.sequence(&TxSequence::zero()).unwrap();
        let input = builder.build().unwrap();
        assert_eq!(input.previous_vout(), 0);
    }

    #[test]
    fn pset_output_builder() {
        use super::PsetOutputBuilder;

        let script = Script::empty();
        let asset: crate::types::AssetId = crate::UniffiCustomTypeConverter::into_custom(
            "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d".to_string(),
        )
        .unwrap();
        let builder = PsetOutputBuilder::new_explicit(&script, 1000, asset, None);
        builder.blinder_index(Some(0)).unwrap();
        let output = builder.build().unwrap();
        assert_eq!(output.amount(), Some(1000));
        assert_eq!(output.blinder_index(), Some(0));
    }
}
