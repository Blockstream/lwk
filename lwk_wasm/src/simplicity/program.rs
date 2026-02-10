//! Simplicity program compilation and execution.

use crate::{Address, ControlBlock, Error, Network, Signer, Transaction, TxOut, XOnlyPublicKey};

use super::arguments::{SimplicityArguments, SimplicityWitnessValues};
use super::cmr::Cmr;
use super::log_level::SimplicityLogLevel;
use super::run_result::SimplicityRunResult;
use super::utils::{convert_utxos, derive_keypair};

use lwk_simplicity::runner;
use lwk_simplicity::scripts;
use lwk_simplicity::signer;
use lwk_simplicity::simplicityhl;

use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::secp256k1::Keypair;
use wasm_bindgen::prelude::*;

/// A compiled Simplicity program ready for use in transactions.
///
/// See [`lwk_simplicity::simplicityhl::CompiledProgram`] for more details.
#[wasm_bindgen]
pub struct SimplicityProgram {
    inner: simplicityhl::CompiledProgram,
}

#[wasm_bindgen]
impl SimplicityProgram {
    /// Load and compile a Simplicity program from source.
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, arguments: &SimplicityArguments) -> Result<SimplicityProgram, Error> {
        let compiled = scripts::load_program(source, arguments.to_inner()?)?;
        Ok(SimplicityProgram { inner: compiled })
    }

    /// Get the Commitment Merkle Root of the program.
    pub fn cmr(&self) -> Cmr {
        self.inner.commit().cmr().into()
    }

    /// Create a P2TR address for this Simplicity program.
    #[wasm_bindgen(js_name = createP2trAddress)]
    pub fn create_p2tr_address(
        &self,
        internal_key: &XOnlyPublicKey,
        network: &Network,
    ) -> Result<Address, Error> {
        let inner_network: lwk_common::Network = network.into();

        let x_only_key = internal_key.to_simplicityhl()?;

        let cmr = self.inner.commit().cmr();
        let address =
            scripts::create_p2tr_address(cmr, &x_only_key, inner_network.address_params());

        Ok(address.into())
    }

    /// Get the taproot control block for script-path spending.
    #[wasm_bindgen(js_name = controlBlock)]
    pub fn control_block(&self, internal_key: &XOnlyPublicKey) -> Result<ControlBlock, Error> {
        let x_only_key = internal_key.to_simplicityhl()?;

        let control_block = scripts::control_block(self.inner.commit().cmr(), x_only_key);

        ControlBlock::new(&control_block.serialize())
    }

    /// Get the sighash_all message for signing a Simplicity program input.
    #[wasm_bindgen(js_name = getSighashAll)]
    pub fn get_sighash_all(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<TxOut>,
        input_index: u32,
        network: &Network,
    ) -> Result<String, Error> {
        let x_only_key = program_public_key.to_simplicityhl()?;
        let utxos_inner = convert_utxos(&utxos);

        let message = signer::get_sighash_all(
            tx.as_ref(),
            &self.inner,
            &x_only_key,
            &utxos_inner,
            input_index as usize,
            network.into(),
        )?;

        // TODO: create a wrapper for the Message type
        Ok(message.as_ref().to_hex())
    }

    /// Finalize a transaction with a Simplicity witness for the specified input.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = finalizeTransaction)]
    pub fn finalize_transaction(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<TxOut>,
        input_index: u32,
        witness_values: &SimplicityWitnessValues,
        network: &Network,
        log_level: SimplicityLogLevel,
    ) -> Result<Transaction, Error> {
        let x_only_key = program_public_key.to_simplicityhl()?;
        let utxos_inner = convert_utxos(&utxos);

        let finalized = signer::finalize_transaction(
            tx.as_ref().clone(),
            &self.inner,
            &x_only_key,
            &utxos_inner,
            input_index as usize,
            witness_values.to_inner()?,
            network.into(),
            log_level.into(),
        )?;

        Ok(finalized.into())
    }

    /// Create a Schnorr signature for a P2PK Simplicity program input.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = createP2pkSignature)]
    pub fn create_p2pk_signature(
        &self,
        signer: &Signer,
        derivation_path: &str,
        tx: &Transaction,
        utxos: Vec<TxOut>,
        input_index: u32,
        network: &Network,
    ) -> Result<String, Error> {
        let keypair_inner: Keypair = derive_keypair(signer, derivation_path)?.into();
        let x_only_pubkey = keypair_inner.x_only_public_key().0;
        let utxos_inner = convert_utxos(&utxos);

        let sighash = signer::get_sighash_all(
            tx.as_ref(),
            &self.inner,
            &x_only_pubkey,
            &utxos_inner,
            input_index as usize,
            network.into(),
        )?;

        let signature = keypair_inner.sign_schnorr(sighash);

        Ok(signature.serialize().to_hex())
    }

    /// Satisfy and execute this program in a transaction environment.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<TxOut>,
        input_index: u32,
        witness_values: &SimplicityWitnessValues,
        network: &Network,
        log_level: SimplicityLogLevel,
    ) -> Result<SimplicityRunResult, Error> {
        let x_only_key = program_public_key.to_simplicityhl()?;
        let utxos_inner = convert_utxos(&utxos);

        let env = signer::get_and_verify_env(
            tx.as_ref(),
            &self.inner,
            &x_only_key,
            &utxos_inner,
            network.into(),
            input_index as usize,
        )?;

        let (pruned, value) = runner::run_program(
            &self.inner,
            witness_values.to_inner()?,
            &env,
            log_level.into(),
        )?;

        Ok(SimplicityRunResult { pruned, value })
    }
}
