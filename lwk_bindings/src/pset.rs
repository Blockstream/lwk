use crate::types::{AssetId, Hex, Tweak};
use crate::{
    Issuance, LwkError, OutPoint, PublicKey, Script, Transaction, TxOut, TxSequence, Txid,
};

use std::fmt::Display;
use std::sync::{Arc, Mutex};

use elements::pset::{Input, Output, PartiallySignedTransaction};
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

/// PSET input
#[derive(uniffi::Object, Debug)]
pub struct PsetInput {
    /// Uses Mutex for in-place mutation. See [`crate::TxBuilder`] for rationale.
    inner: Mutex<Option<Input>>,
}

fn input_consumed() -> LwkError {
    "PsetInput already consumed".into()
}

impl AsRef<Mutex<Option<Input>>> for PsetInput {
    fn as_ref(&self) -> &Mutex<Option<Input>> {
        &self.inner
    }
}

impl From<Input> for PsetInput {
    fn from(inner: Input) -> Self {
        Self {
            inner: Mutex::new(Some(inner)),
        }
    }
}

impl PsetInput {
    fn from_inner(inner: Input) -> Self {
        Self {
            inner: Mutex::new(Some(inner)),
        }
    }
}

#[uniffi::export]
impl PsetInput {
    /// Construct a PsetInput from an outpoint.
    #[uniffi::constructor]
    pub fn from_prevout(outpoint: &OutPoint) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Some(Input::from_prevout(outpoint.into()))),
        })
    }

    /// Set the witness UTXO.
    pub fn set_witness_utxo(&self, utxo: &TxOut) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(input_consumed)?;
        inner.witness_utxo = Some(utxo.into());
        Ok(())
    }

    /// Set the sequence number.
    pub fn set_sequence(&self, sequence: &TxSequence) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(input_consumed)?;
        inner.sequence = Some((*sequence).into());
        Ok(())
    }

    /// Set the issuance value amount.
    pub fn set_issuance_value_amount(&self, amount: u64) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(input_consumed)?;
        inner.issuance_value_amount = Some(amount);
        Ok(())
    }

    /// Set the issuance inflation keys.
    pub fn set_issuance_inflation_keys(&self, amount: u64) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(input_consumed)?;
        inner.issuance_inflation_keys = Some(amount);
        Ok(())
    }

    /// Set the issuance asset entropy.
    pub fn set_issuance_asset_entropy(&self, entropy: &Hex) -> Result<(), LwkError> {
        let bytes: [u8; 32] = entropy.as_ref().try_into()?;
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(input_consumed)?;
        inner.issuance_asset_entropy = Some(bytes);
        Ok(())
    }

    /// Set the blinded issuance flag.
    pub fn set_blinded_issuance(&self, flag: u8) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(input_consumed)?;
        inner.blinded_issuance = Some(flag);
        Ok(())
    }

    /// Set the issuance blinding nonce.
    pub fn set_issuance_blinding_nonce(&self, nonce: &Tweak) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(input_consumed)?;
        inner.issuance_blinding_nonce = Some(nonce.into());
        Ok(())
    }

    /// Prevout TXID of the input.
    pub fn previous_txid(&self) -> Result<Arc<Txid>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(input_consumed)?;
        Ok(Arc::new(inner.previous_txid.into()))
    }

    /// Prevout vout of the input.
    pub fn previous_vout(&self) -> Result<u32, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(input_consumed)?;
        Ok(inner.previous_output_index)
    }

    /// Prevout scriptpubkey of the input.
    pub fn previous_script_pubkey(&self) -> Result<Option<Arc<Script>>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(input_consumed)?;
        Ok(inner
            .witness_utxo
            .as_ref()
            .map(|txout| Arc::new(txout.script_pubkey.clone().into())))
    }

    /// Redeem script of the input.
    pub fn redeem_script(&self) -> Result<Option<Arc<Script>>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(input_consumed)?;
        Ok(inner
            .redeem_script
            .as_ref()
            .map(|s| Arc::new(s.clone().into())))
    }

    /// If the input has an issuance, the asset id.
    pub fn issuance_asset(&self) -> Result<Option<AssetId>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(input_consumed)?;
        Ok(inner.has_issuance().then(|| inner.issuance_ids().0.into()))
    }

    /// If the input has an issuance, the token id.
    pub fn issuance_token(&self) -> Result<Option<AssetId>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(input_consumed)?;
        Ok(inner.has_issuance().then(|| inner.issuance_ids().1.into()))
    }

    /// If the input has a (re)issuance, the issuance object.
    pub fn issuance(&self) -> Result<Option<Arc<Issuance>>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(input_consumed)?;
        Ok(inner
            .has_issuance()
            .then(|| Arc::new(lwk_common::Issuance::new(inner).into())))
    }

    /// Input sighash.
    pub fn sighash(&self) -> Result<u32, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(input_consumed)?;
        Ok(inner.sighash_type.map(|s| s.to_u32()).unwrap_or(1))
    }
}

/// PSET output
#[derive(uniffi::Object, Debug)]
pub struct PsetOutput {
    /// Uses Mutex for in-place mutation. See [`crate::TxBuilder`] for rationale.
    inner: Mutex<Option<Output>>,
}

fn output_consumed() -> LwkError {
    "PsetOutput already consumed".into()
}

impl From<Output> for PsetOutput {
    fn from(inner: Output) -> Self {
        Self {
            inner: Mutex::new(Some(inner)),
        }
    }
}

impl AsRef<Mutex<Option<Output>>> for PsetOutput {
    fn as_ref(&self) -> &Mutex<Option<Output>> {
        &self.inner
    }
}

#[uniffi::export]
impl PsetOutput {
    /// Construct a PsetOutput with explicit asset and value.
    #[uniffi::constructor]
    pub fn new_explicit(
        script_pubkey: &Script,
        satoshi: u64,
        asset: AssetId,
        blinding_key: Option<Arc<PublicKey>>,
    ) -> Arc<Self> {
        let inner = Output {
            script_pubkey: script_pubkey.into(),
            amount: Some(satoshi),
            asset: Some(asset.into()),
            blinding_key: blinding_key.map(|k| k.as_ref().into()),
            ..Default::default()
        };
        Arc::new(Self {
            inner: Mutex::new(Some(inner)),
        })
    }

    /// Set the blinder index.
    pub fn set_blinder_index(&self, index: Option<u32>) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(output_consumed)?;
        inner.blinder_index = index;
        Ok(())
    }

    /// Get the script pubkey.
    pub fn script_pubkey(&self) -> Result<Arc<Script>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(output_consumed)?;
        Ok(Arc::new(inner.script_pubkey.clone().into()))
    }

    /// Get the explicit amount, if set.
    pub fn amount(&self) -> Result<Option<u64>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(output_consumed)?;
        Ok(inner.amount)
    }

    /// Get the explicit asset ID, if set.
    pub fn asset(&self) -> Result<Option<AssetId>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(output_consumed)?;
        Ok(inner.asset.map(Into::into))
    }

    /// Get the blinder index, if set.
    pub fn blinder_index(&self) -> Result<Option<u32>, LwkError> {
        let lock = self.inner.lock()?;
        let inner = lock.as_ref().ok_or_else(output_consumed)?;
        Ok(inner.blinder_index)
    }
}

#[cfg(test)]
mod tests {
    use super::Pset;
    use crate::{OutPoint, Script, TxSequence, Txid};

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
        assert_eq!(pset_in.previous_txid().unwrap(), tx_in.outpoint().txid());
        assert_eq!(pset_in.previous_vout().unwrap(), tx_in.outpoint().vout());
        assert!(pset_in.previous_script_pubkey().unwrap().is_some());
        assert!(pset_in.redeem_script().unwrap().is_none());

        assert!(pset_in.issuance_asset().unwrap().is_none());
        assert!(pset_in.issuance_token().unwrap().is_none());
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
        let txid = Txid::new(
            &"0000000000000000000000000000000000000000000000000000000000000001"
                .parse()
                .unwrap(),
        )
        .unwrap();
        let outpoint = OutPoint::from_parts(&txid, 0);
        let input = super::PsetInput::from_prevout(&outpoint);
        input.set_sequence(&TxSequence::zero()).unwrap();
        assert_eq!(input.previous_vout().unwrap(), 0);
    }

    #[test]
    fn pset_output_builder() {
        let script = Script::empty();
        let asset: crate::types::AssetId = crate::UniffiCustomTypeConverter::into_custom(
            "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d".to_string(),
        )
        .unwrap();
        let output = super::PsetOutput::new_explicit(&script, 1000, asset, None);
        output.set_blinder_index(Some(0)).unwrap();
        assert_eq!(output.amount().unwrap(), Some(1000));
        assert_eq!(output.blinder_index().unwrap(), Some(0));
    }
}
