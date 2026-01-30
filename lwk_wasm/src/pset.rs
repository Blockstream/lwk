use crate::{
    AssetId, ContractHash, Error, Issuance, LockTime, OutPoint, PublicKey, Script, Transaction,
    Tweak, TxOut, TxOutSecrets, TxSequence, Txid,
};

use std::collections::HashMap;
use std::fmt::Display;

use lwk_wollet::elements::hashes::Hash;
use lwk_wollet::elements::pset::{Input, Output, PartiallySignedTransaction};
use lwk_wollet::elements::BlockHash;
use lwk_wollet::elements_miniscript::psbt::finalize;
use lwk_wollet::secp256k1::rand::thread_rng;
use lwk_wollet::EC;

use wasm_bindgen::prelude::*;

/// Partially Signed Elements Transaction
#[wasm_bindgen]
#[derive(PartialEq, Debug, Clone)]
pub struct Pset {
    inner: PartiallySignedTransaction,
}

impl From<PartiallySignedTransaction> for Pset {
    fn from(inner: PartiallySignedTransaction) -> Self {
        Self { inner }
    }
}

impl From<Pset> for PartiallySignedTransaction {
    fn from(pset: Pset) -> Self {
        pset.inner
    }
}

impl Display for Pset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Pset {
    /// Creates a `Pset` from its base64 string representation.
    #[wasm_bindgen(constructor)]
    pub fn new(base64: &str) -> Result<Pset, Error> {
        if base64.trim().is_empty() {
            return Err(Error::Generic("Empty pset".to_string()));
        }
        let pset: PartiallySignedTransaction = base64.trim().parse()?;
        Ok(pset.into())
    }

    /// Return a base64 string representation of the Pset.
    /// The string can be used to re-create the Pset via `new()`
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    /// Finalize and extract the PSET
    pub fn finalize(&self) -> Result<Transaction, Error> {
        let mut pset = self.inner.clone();
        finalize(&mut pset, &EC, BlockHash::all_zeros())?;
        let tx: Transaction = pset.extract_tx()?.into();
        Ok(tx)
    }

    /// Extract the Transaction from a Pset by filling in
    /// the available signature information in place.
    #[wasm_bindgen(js_name = extractTx)]
    pub fn extract_tx(&self) -> Result<Transaction, Error> {
        let tx: Transaction = self.inner.extract_tx()?.into();
        Ok(tx)
    }

    /// Get the unique id of the PSET as defined by [BIP-370](https://github.com/bitcoin/bips/blob/master/bip-0370.mediawiki#unique-identification)
    ///
    /// The unique id is the txid of the PSET with sequence numbers of inputs set to 0
    #[wasm_bindgen(js_name = uniqueId)]
    pub fn unique_id(&self) -> Result<crate::Txid, Error> {
        let txid = self.inner.unique_id()?;
        Ok(txid.into())
    }

    /// Attempt to merge with another `Pset`.
    pub fn combine(&mut self, other: Pset) -> Result<(), Error> {
        self.inner.merge(other.into())?;
        Ok(())
    }

    /// Return a copy of the inputs of this PSET
    pub fn inputs(&self) -> Vec<PsetInput> {
        self.inner.inputs().iter().map(Into::into).collect()
    }

    /// Return a copy of the outputs of this PSET
    pub fn outputs(&self) -> Vec<PsetOutput> {
        self.inner.outputs().iter().map(Into::into).collect()
    }
}

impl Pset {
    // TODO(KyrylR): remove dead_code after all Simplicity features are added
    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> &PartiallySignedTransaction {
        &self.inner
    }
}

/// PSET input
#[wasm_bindgen]
pub struct PsetInput {
    inner: Input,
}

impl From<&Input> for PsetInput {
    fn from(inner: &Input) -> Self {
        Self {
            inner: inner.clone(),
        }
    }
}

impl From<Input> for PsetInput {
    fn from(inner: Input) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl PsetInput {
    /// Prevout TXID of the input
    #[wasm_bindgen(js_name = previousTxid)]
    pub fn previous_txid(&self) -> Txid {
        self.inner.previous_txid.into()
    }

    /// Prevout vout of the input
    #[wasm_bindgen(js_name = previousVout)]
    pub fn previous_vout(&self) -> u32 {
        self.inner.previous_output_index
    }

    /// Prevout scriptpubkey of the input
    #[wasm_bindgen(js_name = previousScriptPubkey)]
    pub fn previous_script_pubkey(&self) -> Option<Script> {
        self.inner
            .witness_utxo
            .as_ref()
            .map(|txout| txout.script_pubkey.clone().into())
    }

    /// Redeem script of the input
    #[wasm_bindgen(js_name = redeemScript)]
    pub fn redeem_script(&self) -> Option<Script> {
        self.inner.redeem_script.as_ref().map(|s| s.clone().into())
    }

    /// If the input has an issuance, the asset id
    #[wasm_bindgen(js_name = issuanceAsset)]
    pub fn issuance_asset(&self) -> Option<AssetId> {
        self.inner
            .has_issuance()
            .then(|| self.inner.issuance_ids().0.into())
    }

    /// If the input has an issuance, the token id
    #[wasm_bindgen(js_name = issuanceToken)]
    pub fn issuance_token(&self) -> Option<AssetId> {
        self.inner
            .has_issuance()
            .then(|| self.inner.issuance_ids().1.into())
    }

    /// If the input has a (re)issuance, the issuance object
    pub fn issuance(&self) -> Option<Issuance> {
        self.inner
            .has_issuance()
            .then(|| lwk_common::Issuance::new(&self.inner).into())
    }

    /// Input sighash
    pub fn sighash(&self) -> u32 {
        self.inner.sighash_type.map(|s| s.to_u32()).unwrap_or(1)
    }

    /// If the input has an issuance, returns [asset_id, token_id].
    /// Returns undefined if the input has no issuance.
    #[wasm_bindgen(js_name = issuanceIds)]
    pub fn issuance_ids(&self) -> Option<Vec<AssetId>> {
        self.inner.has_issuance().then(|| {
            let (asset, token) = self.inner.issuance_ids();
            vec![asset.into(), token.into()]
        })
    }
}

impl PsetInput {
    pub(crate) fn inner(&self) -> &Input {
        &self.inner
    }
}

/// Builder for PSET inputs
#[wasm_bindgen]
pub struct PsetInputBuilder {
    inner: Input,
}

#[wasm_bindgen]
impl PsetInputBuilder {
    /// Construct a PsetInputBuilder from an outpoint.
    #[wasm_bindgen(js_name = fromPrevout)]
    pub fn from_prevout(outpoint: &OutPoint) -> PsetInputBuilder {
        PsetInputBuilder {
            inner: Input::from_prevout(outpoint.into()),
        }
    }

    /// Set the witness UTXO.
    #[wasm_bindgen(js_name = witnessUtxo)]
    pub fn witness_utxo(mut self, utxo: &TxOut) -> PsetInputBuilder {
        self.inner.witness_utxo = Some(utxo.into());
        self
    }

    /// Set the sequence number.
    pub fn sequence(mut self, sequence: &TxSequence) -> PsetInputBuilder {
        self.inner.sequence = Some(sequence.into());
        self
    }

    /// Set the issuance value amount.
    #[wasm_bindgen(js_name = issuanceValueAmount)]
    pub fn issuance_value_amount(mut self, amount: u64) -> PsetInputBuilder {
        self.inner.issuance_value_amount = Some(amount);
        self
    }

    /// Set the issuance inflation keys.
    #[wasm_bindgen(js_name = issuanceInflationKeys)]
    pub fn issuance_inflation_keys(mut self, amount: u64) -> PsetInputBuilder {
        self.inner.issuance_inflation_keys = Some(amount);
        self
    }

    /// Set the issuance asset entropy.
    #[wasm_bindgen(js_name = issuanceAssetEntropy)]
    pub fn issuance_asset_entropy(mut self, contract_hash: &ContractHash) -> PsetInputBuilder {
        let inner_hash: lwk_wollet::elements::ContractHash = contract_hash.into();
        self.inner.issuance_asset_entropy = Some(inner_hash.to_byte_array());
        self
    }

    /// Set the blinded issuance flag.
    #[wasm_bindgen(js_name = blindedIssuance)]
    pub fn blinded_issuance(mut self, flag: bool) -> PsetInputBuilder {
        self.inner.blinded_issuance = Some(u8::from(flag));
        self
    }

    /// Set the issuance blinding nonce.
    #[wasm_bindgen(js_name = issuanceBlindingNonce)]
    pub fn issuance_blinding_nonce(mut self, nonce: &Tweak) -> PsetInputBuilder {
        self.inner.issuance_blinding_nonce = Some(nonce.into());
        self
    }

    /// Build the PsetInput, consuming the builder.
    pub fn build(self) -> PsetInput {
        PsetInput::from(self.inner)
    }
}

/// PSET output
#[wasm_bindgen]
pub struct PsetOutput {
    inner: Output,
}

impl From<&Output> for PsetOutput {
    fn from(inner: &Output) -> Self {
        Self {
            inner: inner.clone(),
        }
    }
}

impl From<Output> for PsetOutput {
    fn from(inner: Output) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl PsetOutput {
    /// Get the script pubkey
    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(&self) -> Script {
        self.inner.script_pubkey.clone().into()
    }

    /// Get the explicit amount, if set
    pub fn amount(&self) -> Option<u64> {
        self.inner.amount
    }

    /// Get the explicit asset ID, if set
    pub fn asset(&self) -> Option<AssetId> {
        self.inner.asset.map(Into::into)
    }

    /// Get the blinder index, if set
    #[wasm_bindgen(js_name = blinderIndex)]
    pub fn blinder_index(&self) -> Option<u32> {
        self.inner.blinder_index
    }
}

impl PsetOutput {
    pub(crate) fn inner(&self) -> &Output {
        &self.inner
    }
}

/// Builder for PSET outputs
#[wasm_bindgen]
pub struct PsetOutputBuilder {
    inner: Output,
}

#[wasm_bindgen]
impl PsetOutputBuilder {
    /// Construct a PsetOutputBuilder with explicit asset and value.
    #[wasm_bindgen(js_name = newExplicit)]
    pub fn new_explicit(
        script_pubkey: &Script,
        satoshi: u64,
        asset: &AssetId,
    ) -> PsetOutputBuilder {
        let output = Output {
            script_pubkey: script_pubkey.as_ref().clone(),
            amount: Some(satoshi),
            asset: Some((*asset).into()),
            ..Default::default()
        };
        PsetOutputBuilder { inner: output }
    }

    /// Set the blinding public key.
    #[wasm_bindgen(js_name = blindingPubkey)]
    pub fn blinding_pubkey(mut self, blinding_key: &PublicKey) -> PsetOutputBuilder {
        self.inner.blinding_key = Some(blinding_key.into());
        self
    }

    /// Set the script pubkey.
    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(mut self, script_pubkey: &Script) -> PsetOutputBuilder {
        self.inner.script_pubkey = script_pubkey.as_ref().clone();
        self
    }

    /// Set the explicit amount.
    pub fn satoshi(mut self, satoshi: u64) -> PsetOutputBuilder {
        self.inner.amount = Some(satoshi);
        self
    }

    /// Set the explicit asset ID.
    pub fn asset(mut self, asset: &AssetId) -> PsetOutputBuilder {
        self.inner.asset = Some((*asset).into());
        self
    }

    /// Set the blinder index.
    #[wasm_bindgen(js_name = blinderIndex)]
    pub fn blinder_index(mut self, index: u32) -> PsetOutputBuilder {
        self.inner.blinder_index = Some(index);
        self
    }

    /// Build the PsetOutput, consuming the builder.
    pub fn build(self) -> PsetOutput {
        PsetOutput::from(self.inner)
    }
}

/// Builder for constructing a PSET from scratch
#[wasm_bindgen]
pub struct PsetBuilder {
    inner: PartiallySignedTransaction,
}

#[wasm_bindgen]
impl PsetBuilder {
    /// Create a new PSET v2 builder
    #[wasm_bindgen(js_name = newV2)]
    pub fn new_v2() -> PsetBuilder {
        PsetBuilder {
            inner: PartiallySignedTransaction::new_v2(),
        }
    }

    /// Add an input to this PSET
    #[wasm_bindgen(js_name = addInput)]
    pub fn add_input(mut self, input: &PsetInput) -> PsetBuilder {
        self.inner.add_input(input.inner().clone());
        self
    }

    /// Add an output to this PSET
    #[wasm_bindgen(js_name = addOutput)]
    pub fn add_output(mut self, output: &PsetOutput) -> PsetBuilder {
        self.inner.add_output(output.inner().clone());
        self
    }

    /// Set the fallback locktime on the PSET global tx_data
    #[wasm_bindgen(js_name = setFallbackLocktime)]
    pub fn set_fallback_locktime(mut self, locktime: &LockTime) -> PsetBuilder {
        self.inner.global.tx_data.fallback_locktime = Some(locktime.into());
        self
    }

    /// Blind the last output using the provided input secrets.
    ///
    /// `inp_txout_sec` is a map from input index to TxOutSecrets, represented as
    /// parallel arrays where `input_indices[i]` corresponds to `secrets[i]`.
    #[wasm_bindgen(js_name = blindLast)]
    pub fn blind_last(
        mut self,
        input_indices: &[u32],
        secrets: Vec<TxOutSecrets>,
    ) -> Result<PsetBuilder, Error> {
        if input_indices.len() != secrets.len() {
            return Err(Error::Generic(format!(
                "input_indices length ({}) must match secrets length ({})",
                input_indices.len(),
                secrets.len()
            )));
        }

        let converted: HashMap<usize, lwk_wollet::elements::TxOutSecrets> = input_indices
            .iter()
            .zip(secrets.iter())
            .map(|(&idx, sec)| (idx as usize, sec.into()))
            .collect();

        self.inner.blind_last(&mut thread_rng(), &EC, &converted)?;
        Ok(self)
    }

    /// Build the Pset, consuming the builder
    pub fn build(self) -> Pset {
        Pset::from(self.inner)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn pset_roundtrip() {
        let pset_string =
            include_str!("../../lwk_jade/test_data/pset_to_be_signed.base64").to_string();
        let pset = Pset::new(&pset_string).unwrap();

        let tx_expected =
            include_str!("../../lwk_jade/test_data/pset_to_be_signed_transaction.hex").to_string();
        let tx = pset.extract_tx().unwrap();
        assert_eq!(tx_expected, tx.to_string());

        assert_eq!(pset_string, pset.to_string());

        let pset_in = &pset.inputs()[0];
        assert_eq!(
            pset_in.previous_txid().to_string(),
            "0093c96a69e9ea00b5409611f23435b6639c157afa1c88cf18960715ea10116c"
        );
        assert_eq!(pset_in.previous_vout(), 0);
        assert!(pset_in.previous_script_pubkey().is_some());
        assert!(pset_in.redeem_script().is_none());

        assert!(pset_in.issuance_asset().is_none());
        assert!(pset_in.issuance_token().is_none());
    }

    #[wasm_bindgen_test]
    fn pset_builder() {
        let txid =
            Txid::new("0000000000000000000000000000000000000000000000000000000000000001").unwrap();
        let outpoint = OutPoint::from_parts(&txid, 0);
        let input = PsetInputBuilder::from_prevout(&outpoint)
            .sequence(&TxSequence::zero())
            .build();
        assert_eq!(input.previous_vout(), 0);

        let script = Script::empty();
        let asset =
            AssetId::new("6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d")
                .unwrap();
        let output = PsetOutputBuilder::new_explicit(&script, 1000, &asset)
            .blinder_index(0)
            .build();
        assert_eq!(output.amount(), Some(1000));
        assert_eq!(output.blinder_index(), Some(0));

        let locktime = LockTime::from_height(100).unwrap();
        let pset = PsetBuilder::new_v2()
            .add_input(&input)
            .add_output(&output)
            .set_fallback_locktime(&locktime)
            .build();
        assert_eq!(pset.inputs().len(), 1);
        assert_eq!(pset.outputs().len(), 1);
        assert_eq!(pset.outputs()[0].amount(), Some(1000));
        assert_eq!(
            pset.inner().global.tx_data.fallback_locktime,
            Some(lwk_wollet::elements::LockTime::from_consensus(100))
        );
    }
}
