use std::collections::HashMap;

use crate::error::PsetBuilderError;
use crate::signer;
use crate::utils::network_to_address_params;

use simplicityhl::elements::bitcoin::secp256k1::{Keypair, SecretKey, SECP256K1};
use simplicityhl::elements::bitcoin::PublicKey;
use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use simplicityhl::elements::pset::{Input, Output, PartiallySignedTransaction};
use simplicityhl::elements::secp256k1_zkp::rand::thread_rng;
use simplicityhl::elements::{AssetId, OutPoint, Script, Sequence, TxOut, TxOutSecrets};

use lwk_common::{calculate_fee, DEFAULT_FEE_RATE};
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::tracker::TrackerLogLevel;
use simplicityhl::{CompiledProgram, WitnessValues};

/// Placeholder fee for first pass of two-pass fee estimation
pub const PLACEHOLDER_FEE: u64 = 1;

#[derive(Clone)]
pub struct SimplicityInputSpec {
    pub program: CompiledProgram,
    pub internal_key: XOnlyPublicKey,
    pub input_index: usize,
}

/// Input for the SimplicityPsetBuilder containing program, keys, and UTXO data.
#[derive(Clone)]
pub struct SimplicityPsetInput {
    /// The Simplicity input specification (program, internal key, input index).
    pub spec: SimplicityInputSpec,
    /// The outpoint being spent.
    pub outpoint: OutPoint,
    /// The UTXO being spent.
    pub utxo: TxOut,
}

/// Output specification for the SimplicityPsetBuilder.
#[derive(Clone)]
pub enum SimplicityOutputSpec {
    /// Send L-BTC to an address.
    LbtcRecipient {
        address: elements::Address,
        satoshi: u64,
    },
    /// Send a specific asset to an address.
    AssetRecipient {
        address: elements::Address,
        satoshi: u64,
        asset: AssetId,
    },
    /// Send remaining L-BTC (change) to an address.
    DrainLbtcTo { address: elements::Address },
}

/// A PSET-based transaction builder for Simplicity programs.
///
/// This builder handles:
/// - Input/output management
/// - Fee calculation with two-pass estimation
/// - PSET construction
/// - Signing via callback
pub struct SimplicityPsetBuilder {
    network: lwk_common::Network,
    policy_asset: AssetId,
    genesis_hash: elements::BlockHash,
    inputs: Vec<SimplicityPsetInput>,
    outputs: Vec<SimplicityOutputSpec>,
    fee_rate: f32,
}

impl SimplicityPsetBuilder {
    /// Create a new SimplicityPsetBuilder.
    #[must_use]
    pub fn new(
        network: lwk_common::Network,
        policy_asset: AssetId,
        genesis_hash: elements::BlockHash,
    ) -> Self {
        Self {
            network,
            policy_asset,
            genesis_hash,
            inputs: Vec::new(),
            outputs: Vec::new(),
            fee_rate: DEFAULT_FEE_RATE,
        }
    }

    /// Set the fee rate in sats/kvb.
    pub fn set_fee_rate(&mut self, rate: f32) {
        self.fee_rate = rate;
    }

    /// Get the fee rate in sats/kvb.
    #[must_use]
    pub fn fee_rate(&self) -> f32 {
        self.fee_rate
    }

    /// Add a Simplicity input.
    pub fn add_input(&mut self, input: SimplicityPsetInput) {
        self.inputs.push(input);
    }

    /// Add an output specification.
    pub fn add_output(&mut self, output: SimplicityOutputSpec) {
        self.outputs.push(output);
    }

    /// Get the number of inputs.
    #[must_use]
    pub fn n_inputs(&self) -> usize {
        self.inputs.len()
    }

    /// Get the UTXOs for all inputs.
    #[must_use]
    pub fn utxos(&self) -> Vec<TxOut> {
        self.inputs.iter().map(|i| i.utxo.clone()).collect()
    }

    /// Get the input specifications.
    #[must_use]
    pub fn inputs(&self) -> &[SimplicityPsetInput] {
        &self.inputs
    }

    /// Get the network.
    #[must_use]
    pub fn network(&self) -> lwk_common::Network {
        self.network
    }

    /// Get the genesis hash.
    #[must_use]
    pub fn genesis_hash(&self) -> elements::BlockHash {
        self.genesis_hash
    }

    /// Build an unsigned PSET with the given fee.
    ///
    /// # Errors
    /// Returns error if inputs are empty or values overflow.
    pub fn build_pset(&self, fee: u64) -> Result<PartiallySignedTransaction, PsetBuilderError> {
        if self.inputs.is_empty() {
            return Err(PsetBuilderError::NoInputs);
        }

        let mut builder = PsetBuilder::new_v2();

        // Add inputs
        for input_data in &self.inputs {
            let pset_input = PsetInputBuilder::from_outpoint(input_data.outpoint)
                .witness_utxo(input_data.utxo.clone())
                .build();
            builder.add_input(pset_input);
        }

        // Calculate total L-BTC input
        let mut total_lbtc_input: u64 = 0;
        for input_data in &self.inputs {
            if let Some(asset) = input_data.utxo.asset.explicit() {
                if asset == self.policy_asset {
                    if let Some(value) = input_data.utxo.value.explicit() {
                        total_lbtc_input = total_lbtc_input
                            .checked_add(value)
                            .ok_or(PsetBuilderError::InputValueOverflow)?;
                    }
                }
            }
        }

        // Process outputs
        let mut total_lbtc_output: u64 = 0;
        let mut drain_address: Option<elements::Address> = None;

        for output_spec in &self.outputs {
            match output_spec {
                SimplicityOutputSpec::LbtcRecipient { address, satoshi } => {
                    let output = PsetOutputBuilder::new_explicit(
                        address.script_pubkey(),
                        *satoshi,
                        self.policy_asset,
                        None,
                    )
                    .build();
                    builder.add_output(output);
                    total_lbtc_output = total_lbtc_output
                        .checked_add(*satoshi)
                        .ok_or(PsetBuilderError::OutputValueOverflow)?;
                }
                SimplicityOutputSpec::AssetRecipient {
                    address,
                    satoshi,
                    asset,
                } => {
                    let output = PsetOutputBuilder::new_explicit(
                        address.script_pubkey(),
                        *satoshi,
                        *asset,
                        None,
                    )
                    .build();
                    builder.add_output(output);
                    if *asset == self.policy_asset {
                        total_lbtc_output = total_lbtc_output
                            .checked_add(*satoshi)
                            .ok_or(PsetBuilderError::OutputValueOverflow)?;
                    }
                }
                SimplicityOutputSpec::DrainLbtcTo { address } => {
                    drain_address = Some(address.clone());
                }
            }
        }

        // Add fee output
        let fee_output = PsetOutputBuilder::new_fee(fee, self.policy_asset).build();
        builder.add_output(fee_output);
        total_lbtc_output = total_lbtc_output
            .checked_add(fee)
            .ok_or(PsetBuilderError::FeeOverflow)?;

        // Add change output if needed
        if let Some(change_addr) = drain_address {
            if total_lbtc_input > total_lbtc_output {
                let change = total_lbtc_input - total_lbtc_output;
                if change > 0 {
                    let change_output = PsetOutputBuilder::new_explicit(
                        change_addr.script_pubkey(),
                        change,
                        self.policy_asset,
                        None,
                    )
                    .build();
                    builder.add_output(change_output);
                }
            }
        } else if total_lbtc_input < total_lbtc_output {
            return Err(PsetBuilderError::InsufficientFunds {
                have: total_lbtc_input,
                need: total_lbtc_output,
                fee,
            });
        }

        Ok(builder.into_inner())
    }

    /// Sign and finalize a PSET using a callback function.
    ///
    /// The callback is called for each input and should return the witness values.
    ///
    /// # Arguments
    /// * `pset` - The PSET to sign
    /// * `signer_callback` - Callback that takes (input_index, spec, sighash) and returns WitnessValues
    /// * `log_level` - Log level for Simplicity program execution
    ///
    /// # Errors
    /// Returns error if signing or finalization fails.
    pub fn sign_and_finalize_pset<F>(
        &self,
        mut pset: PartiallySignedTransaction,
        signer_callback: F,
        log_level: TrackerLogLevel,
    ) -> Result<PartiallySignedTransaction, PsetBuilderError>
    where
        F: Fn(
            usize,
            &SimplicityInputSpec,
            &elements::secp256k1_zkp::Message,
        ) -> Result<WitnessValues, PsetBuilderError>,
    {
        let utxos = self.utxos();

        for (input_idx, input_data) in self.inputs.iter().enumerate() {
            let sighash = get_simplicity_sighash(
                &pset,
                &input_data.spec,
                &utxos,
                self.network,
                self.genesis_hash,
            )?;

            let witness = signer_callback(input_idx, &input_data.spec, &sighash)?;

            pset = finalize_simplicity_input(
                pset,
                &input_data.spec,
                &utxos,
                witness,
                self.network,
                self.genesis_hash,
                log_level,
            )?;
        }

        Ok(pset)
    }

    /// Build, sign, and finalize the transaction with two-pass fee estimation.
    ///
    /// This method:
    /// 1. Builds a PSET with placeholder fee
    /// 2. Signs it to get accurate weight
    /// 3. Calculates the actual fee from weight
    /// 4. Rebuilds and signs with the actual fee
    ///
    /// # Arguments
    /// * `signer_callback` - Callback that takes (input_index, spec, sighash) and returns WitnessValues
    /// * `log_level` - Log level for Simplicity program execution
    ///
    /// # Errors
    /// Returns error if building, signing, or finalization fails.
    pub fn finish<F>(
        &self,
        signer_callback: F,
        log_level: TrackerLogLevel,
    ) -> Result<PartiallySignedTransaction, PsetBuilderError>
    where
        F: Fn(
            usize,
            &SimplicityInputSpec,
            &elements::secp256k1_zkp::Message,
        ) -> Result<WitnessValues, PsetBuilderError>,
    {
        // First pass: build with placeholder fee to get weight
        let pset1 = self.build_pset(PLACEHOLDER_FEE)?;
        let signed1 = self.sign_and_finalize_pset(pset1, &signer_callback, log_level)?;
        let tx1 = signed1
            .extract_tx()
            .map_err(|e| PsetBuilderError::Extract(e.to_string()))?;
        let weight = tx1.weight();
        let fee = calculate_fee(weight, self.fee_rate);

        // Second pass: build with actual fee
        let pset2 = self.build_pset(fee)?;
        let signed2 = self.sign_and_finalize_pset(pset2, &signer_callback, log_level)?;

        Ok(signed2)
    }
}

pub fn finalize_simplicity_input(
    pset: PartiallySignedTransaction,
    spec: &SimplicityInputSpec,
    utxos: &[TxOut],
    witness_values: simplicityhl::WitnessValues,
    network: lwk_common::Network,
    genesis_hash: elements::BlockHash,
    log_level: TrackerLogLevel,
) -> Result<PartiallySignedTransaction, PsetBuilderError> {
    let params = network_to_address_params(network);

    let mut tx = pset
        .extract_tx()
        .map_err(|e| PsetBuilderError::Extract(e.to_string()))?;

    tx = signer::finalize_transaction(
        tx,
        &spec.program,
        &spec.internal_key,
        utxos,
        spec.input_index,
        witness_values,
        params,
        genesis_hash,
        log_level,
    )?;

    let mut new_pset = PartiallySignedTransaction::from_tx(tx);
    for (i, utxo) in utxos.iter().enumerate() {
        if i < new_pset.inputs().len() {
            new_pset.inputs_mut()[i].witness_utxo = Some(utxo.clone());
        }
    }

    Ok(new_pset)
}

pub fn get_simplicity_sighash(
    pset: &PartiallySignedTransaction,
    spec: &SimplicityInputSpec,
    utxos: &[TxOut],
    network: lwk_common::Network,
    genesis_hash: elements::BlockHash,
) -> Result<elements::secp256k1_zkp::Message, PsetBuilderError> {
    let params = network_to_address_params(network);

    let tx = pset
        .extract_tx()
        .map_err(|e| PsetBuilderError::Sighash(e.to_string()))?;

    signer::get_sighash_all(
        &tx,
        &spec.program,
        &spec.internal_key,
        utxos,
        spec.input_index,
        params,
        genesis_hash,
    )
    .map_err(|e| PsetBuilderError::Sighash(e.to_string()))
}

/// Builder for PSET inputs with issuance/reissuance support.
#[derive(Debug, Clone)]
pub struct PsetInputBuilder {
    input: Input,
}

impl PsetInputBuilder {
    /// Create a new input builder from an outpoint.
    #[must_use]
    pub fn from_outpoint(outpoint: OutPoint) -> Self {
        Self {
            input: Input::from_prevout(outpoint),
        }
    }

    /// Set the witness UTXO for this input.
    #[must_use]
    pub fn witness_utxo(mut self, utxo: TxOut) -> Self {
        self.input.witness_utxo = Some(utxo);
        self
    }

    /// Set the sequence number.
    #[must_use]
    pub fn sequence(mut self, sequence: Sequence) -> Self {
        self.input.sequence = Some(sequence);
        self
    }

    /// Set issuance value amount (for reissuance).
    #[must_use]
    pub fn issuance_value_amount(mut self, amount: u64) -> Self {
        self.input.issuance_value_amount = Some(amount);
        self
    }

    /// Set issuance asset entropy.
    #[must_use]
    pub fn issuance_asset_entropy(mut self, entropy: [u8; 32]) -> Self {
        self.input.issuance_asset_entropy = Some(entropy);
        self
    }

    /// Set issuance blinding nonce (asset blinding factor from UTXO secrets).
    #[must_use]
    pub fn issuance_blinding_nonce(mut self, abf: AssetBlindingFactor) -> Self {
        self.input.issuance_blinding_nonce = Some(abf.into_inner());
        self
    }

    /// Set blinded issuance flag (0x00 for unblinded, 0x01 for blinded).
    #[must_use]
    pub fn blinded_issuance(mut self, flag: u8) -> Self {
        self.input.blinded_issuance = Some(flag);
        self
    }

    /// Set issuance inflation keys amount.
    #[must_use]
    pub fn issuance_inflation_keys(mut self, amount: Option<u64>) -> Self {
        self.input.issuance_inflation_keys = amount;
        self
    }

    /// Build the input.
    #[must_use]
    pub fn build(self) -> Input {
        self.input
    }
}

/// Builder for PSET outputs.
#[derive(Debug, Clone)]
pub struct PsetOutputBuilder {
    output: Output,
}

impl PsetOutputBuilder {
    /// Create a new explicit output (unblinded).
    #[must_use]
    pub fn new_explicit(
        script_pubkey: Script,
        amount: u64,
        asset: AssetId,
        blinding_pubkey: Option<PublicKey>,
    ) -> Self {
        Self {
            output: Output::new_explicit(script_pubkey, amount, asset, blinding_pubkey),
        }
    }

    /// Create a fee output.
    #[must_use]
    pub fn new_fee(amount: u64, asset: AssetId) -> Self {
        Self {
            output: Output::new_explicit(Script::new(), amount, asset, None),
        }
    }

    /// Set the blinder index (which input provides blinding entropy).
    #[must_use]
    pub fn blinder_index(mut self, index: u32) -> Self {
        self.output.blinder_index = Some(index);
        self
    }

    /// Build the output.
    #[must_use]
    pub fn build(self) -> Output {
        self.output
    }
}

/// PSET builder with blinding support.
pub struct PsetBuilder {
    pset: PartiallySignedTransaction,
}

impl PsetBuilder {
    /// Create a new empty PSET (version 2).
    #[must_use]
    pub fn new_v2() -> Self {
        Self {
            pset: PartiallySignedTransaction::new_v2(),
        }
    }

    /// Add an input to the PSET.
    pub fn add_input(&mut self, input: Input) {
        self.pset.add_input(input);
    }

    /// Add an output to the PSET.
    pub fn add_output(&mut self, output: Output) {
        self.pset.add_output(output);
    }

    /// Get the number of inputs.
    #[must_use]
    pub fn n_inputs(&self) -> usize {
        self.pset.n_inputs()
    }

    /// Get the number of outputs.
    #[must_use]
    pub fn n_outputs(&self) -> usize {
        self.pset.n_outputs()
    }

    /// Blind the last outputs of the PSET.
    ///
    /// # Arguments
    /// * `inp_txout_sec` - Map of input index to TxOutSecrets for blinding
    pub fn blind_last(
        &mut self,
        inp_txout_sec: &HashMap<usize, TxOutSecrets>,
    ) -> Result<(), PsetBuilderError> {
        self.pset
            .blind_last(&mut thread_rng(), SECP256K1, inp_txout_sec)
            .map_err(|e| PsetBuilderError::Blinding(e.to_string()))
    }

    /// Extract the transaction from the PSET.
    pub fn extract_tx(&self) -> Result<elements::Transaction, PsetBuilderError> {
        self.pset
            .extract_tx()
            .map_err(|e| PsetBuilderError::Extract(e.to_string()))
    }

    /// Get a reference to the inner PSET.
    #[must_use]
    pub fn inner(&self) -> &PartiallySignedTransaction {
        &self.pset
    }

    /// Consume and return the inner PSET.
    #[must_use]
    pub fn into_inner(self) -> PartiallySignedTransaction {
        self.pset
    }
}

impl Default for PsetBuilder {
    fn default() -> Self {
        Self::new_v2()
    }
}

impl From<PartiallySignedTransaction> for PsetBuilder {
    fn from(pset: PartiallySignedTransaction) -> Self {
        Self { pset }
    }
}

/// Generate a new random blinding keypair.
#[must_use]
pub fn generate_blinding_keypair() -> Keypair {
    Keypair::new(SECP256K1, &mut thread_rng())
}

/// Get the public key from a secret key.
#[must_use]
pub fn public_key_from_secret(secret_key: &SecretKey) -> PublicKey {
    let secp_pubkey =
        elements::bitcoin::secp256k1::PublicKey::from_secret_key(SECP256K1, secret_key);
    PublicKey::new(secp_pubkey)
}

/// Extract blinding factors from TxOutSecrets as byte arrays.
#[must_use]
pub fn blinding_factors_from_secrets(secrets: &TxOutSecrets) -> ([u8; 32], [u8; 32]) {
    (
        *secrets.asset_bf.into_inner().as_ref(),
        *secrets.value_bf.into_inner().as_ref(),
    )
}

/// Create TxOutSecrets from components.
#[must_use]
pub fn tx_out_secrets_new(
    asset: AssetId,
    asset_bf: [u8; 32],
    value: u64,
    value_bf: [u8; 32],
) -> TxOutSecrets {
    TxOutSecrets::new(
        asset,
        AssetBlindingFactor::from_slice(&asset_bf).expect("valid 32 bytes"),
        value,
        ValueBlindingFactor::from_slice(&value_bf).expect("valid 32 bytes"),
    )
}

/// Create explicit (unblinded) TxOutSecrets.
#[must_use]
pub fn tx_out_secrets_explicit(asset: AssetId, value: u64) -> TxOutSecrets {
    TxOutSecrets::new(
        asset,
        AssetBlindingFactor::zero(),
        value,
        ValueBlindingFactor::zero(),
    )
}

/// Verify transaction amount proofs against UTXOs.
pub fn verify_tx_amt_proofs(
    tx: &elements::Transaction,
    utxos: &[TxOut],
) -> Result<(), PsetBuilderError> {
    tx.verify_tx_amt_proofs(SECP256K1, utxos)
        .map_err(|e| PsetBuilderError::VerifyProofs(e.to_string()))
}

/// Unblind a transaction output.
pub fn unblind_output(
    output: &TxOut,
    blinding_key: &SecretKey,
) -> Result<TxOutSecrets, PsetBuilderError> {
    output
        .unblind(SECP256K1, *blinding_key)
        .map_err(|e| PsetBuilderError::Blinding(format!("Failed to unblind: {e}")))
}

/// Helper to get explicit asset and value from a TxOut.
///
/// Returns `None` if the output is confidential.
#[must_use]
pub fn tx_out_explicit(tx_out: &TxOut) -> Option<(AssetId, u64)> {
    match (tx_out.asset.explicit(), tx_out.value.explicit()) {
        (Some(asset), Some(value)) => Some((asset, value)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pset_builder_new() {
        let builder = PsetBuilder::new_v2();
        assert_eq!(builder.n_inputs(), 0);
        assert_eq!(builder.n_outputs(), 0);
    }

    #[test]
    fn test_input_builder() {
        let outpoint = OutPoint::default();
        let input = PsetInputBuilder::from_outpoint(outpoint)
            .sequence(Sequence::ZERO)
            .issuance_value_amount(1000)
            .issuance_asset_entropy([0u8; 32])
            .blinded_issuance(0x00)
            .build();

        assert_eq!(input.sequence, Some(Sequence::ZERO));
        assert_eq!(input.issuance_value_amount, Some(1000));
        assert_eq!(input.blinded_issuance, Some(0x00));
    }

    #[test]
    fn test_output_builder() {
        let asset = AssetId::from_slice(&[1u8; 32]).unwrap();
        let output = PsetOutputBuilder::new_explicit(Script::new(), 1000, asset, None)
            .blinder_index(0)
            .build();

        assert_eq!(output.blinder_index, Some(0));
    }

    #[test]
    fn test_generate_blinding_keypair() {
        let keypair = generate_blinding_keypair();
        let pubkey = keypair.public_key();
        let secret = keypair.secret_key();

        // Verify we can derive public from secret
        let derived_pubkey = public_key_from_secret(&secret);
        assert_eq!(pubkey, derived_pubkey.inner);
    }

    #[test]
    fn test_tx_out_secrets_explicit() {
        let asset = AssetId::from_slice(&[1u8; 32]).unwrap();
        let secrets = tx_out_secrets_explicit(asset, 1000);

        assert_eq!(secrets.asset, asset);
        assert_eq!(secrets.value, 1000);
        assert_eq!(secrets.asset_bf, AssetBlindingFactor::zero());
        assert_eq!(secrets.value_bf, ValueBlindingFactor::zero());
    }
}
