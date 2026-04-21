//! Simplicity program compilation and execution.

use crate::{Address, ControlBlock, Error, Network, Signer, Transaction, TxOut };

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
#[wasm_bindgen]
pub struct SimplicityProgram {
    inner: simplicityhl::CompiledProgram,
}

#[wasm_bindgen]
impl SimplicityProgram {
    /// Load and compile a Simplicity program from source.
    pub fn load(source: &str, arguments: &SimplicityArguments) -> Result<SimplicityProgram, Error> {
        let compiled = scripts::load_program(source, arguments.to_inner()?)?;
        Ok(SimplicityProgram { inner: compiled })
    }

    /// Get the Commitment Merkle Root of the program.
    #[wasm_bindgen(getter = cmr)]
    pub fn cmr(&self) -> Cmr {
        self.inner.commit().cmr().into()
    }

    /// Create a P2TR address for this Simplicity program.
    #[wasm_bindgen(js_name = createP2trAddress)]
    pub fn create_p2tr_address(
        &self,
        network: &Network,
    ) -> Result<Address, Error> {
        let inner_network: lwk_common::Network = network.into();


        let cmr = self.inner.commit().cmr();
        let address =
            scripts::create_p2tr_address(cmr, inner_network.address_params());

        Ok(address.into())
    }

    /// Get the taproot control block for script-path spending.
    #[wasm_bindgen(js_name = controlBlock)]
    pub fn control_block(&self) -> Result<ControlBlock, Error> {

        let control_block = scripts::control_block(self.inner.commit().cmr());

        ControlBlock::from_bytes(&control_block.serialize())
    }

    /// Get the sighash_all message for signing a Simplicity program input.
    ///
    /// NOTE: The utxos object is destroyed during the execution of the function, so the argument that was
    /// passed in the JS code cannot be reused.
    // TODO: address the limitation
    #[wasm_bindgen(js_name = getSighashAll)]
    pub fn get_sighash_all(
        &self,
        tx: &Transaction,
        utxos: Vec<TxOut>,
        input_index: u32,
        network: &Network,
    ) -> Result<String, Error> {
        let utxos_inner = convert_utxos(&utxos);

        let message = signer::get_sighash_all(
            tx.as_ref(),
            &self.inner,
            &utxos_inner,
            input_index as usize,
            network.into(),
        )?;

        // TODO: create a wrapper for the Message type
        Ok(message.as_ref().to_hex())
    }

    /// Finalize a transaction with a Simplicity witness for the specified input.
    ///
    /// NOTE: The utxos object is destroyed during the execution of the function, so the argument that was
    /// passed in the JS code cannot be reused.
    // TODO: address the limitation
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = finalizeTransaction)]
    pub fn finalize_transaction(
        &self,
        tx: &Transaction,
        utxos: Vec<TxOut>,
        input_index: u32,
        witness_values: &SimplicityWitnessValues,
        network: &Network,
        log_level: SimplicityLogLevel,
    ) -> Result<Transaction, Error> {
        let utxos_inner = convert_utxos(&utxos);

        let finalized = signer::finalize_transaction(
            tx.as_ref().clone(),
            &self.inner,
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
        let utxos_inner = convert_utxos(&utxos);

        let sighash = signer::get_sighash_all(
            tx.as_ref(),
            &self.inner,
            &utxos_inner,
            input_index as usize,
            network.into(),
        )?;

        let signature = keypair_inner.sign_schnorr(sighash);

        Ok(signature.serialize().to_hex())
    }

    /// Satisfy and execute this program in a transaction environment.
    ///
    /// NOTE: The utxos object is destroyed during the execution of the function, so the argument that was
    /// passed in the JS code cannot be reused.
    // TODO: address the limitation
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        tx: &Transaction,
        utxos: Vec<TxOut>,
        input_index: u32,
        witness_values: &SimplicityWitnessValues,
        network: &Network,
        log_level: SimplicityLogLevel,
    ) -> Result<SimplicityRunResult, Error> {
        let utxos_inner = convert_utxos(&utxos);

        let env = signer::get_and_verify_env(
            tx.as_ref(),
            &self.inner,
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
