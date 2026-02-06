use std::sync::Arc;

use lwk_simplicity::runner;
use lwk_simplicity::scripts;
use lwk_simplicity::signer;
use lwk_simplicity::simplicityhl;

use crate::blockdata::tx_out::TxOut;
use crate::types::{Hex, XOnlyPublicKey};
use crate::{Address, LwkError, Network, Transaction};

use super::arguments::{SimplicityArguments, SimplicityWitnessValues};
use super::log_level::SimplicityLogLevel;
use super::run_result::SimplicityRunResult;
use super::utils::{convert_utxos, derive_keypair};

/// A compiled Simplicity program ready for use in transactions.
///
/// See [`lwk_simplicity::simplicityhl::CompiledProgram`] for more details.
#[derive(uniffi::Object)]
pub struct SimplicityProgram {
    inner: simplicityhl::CompiledProgram,
}

#[uniffi::export]
impl SimplicityProgram {
    /// Load and compile a Simplicity program from source.
    #[uniffi::constructor]
    pub fn load(source: String, arguments: &SimplicityArguments) -> Result<Arc<Self>, LwkError> {
        let compiled = scripts::load_program(&source, arguments.to_inner())?;
        Ok(Arc::new(SimplicityProgram { inner: compiled }))
    }

    /// Get the Commitment Merkle Root (CMR) of the program as hex.
    pub fn cmr(&self) -> Hex {
        let cmr = self.inner.commit().cmr();
        Hex::from(cmr.as_ref().to_vec())
    }

    /// Create a P2TR (Pay-to-Taproot) address for this Simplicity program.
    pub fn create_p2tr_address(
        &self,
        internal_key: &XOnlyPublicKey,
        network: &Network,
    ) -> Result<Arc<Address>, LwkError> {
        let x_only_key = internal_key.to_simplicityhl()?;

        let inner_network: lwk_common::Network = network.into();
        let cmr = self.inner.commit().cmr();
        let address =
            scripts::create_p2tr_address(cmr, &x_only_key, inner_network.address_params());

        Ok(Arc::new(address.into()))
    }

    /// Get the taproot control block for script-path spending.
    pub fn control_block(&self, internal_key: &XOnlyPublicKey) -> Result<Hex, LwkError> {
        let x_only_key = internal_key.to_simplicityhl()?;

        let cmr = self.inner.commit().cmr();
        let control_block = scripts::control_block(cmr, x_only_key);

        Ok(Hex::from(control_block.serialize()))
    }

    /// Get the sighash_all message for signing a Simplicity program input.
    pub fn get_sighash_all(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<Arc<TxOut>>,
        input_index: u32,
        network: &Network,
    ) -> Result<Hex, LwkError> {
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

        Ok(Hex::from(message.as_ref().to_vec()))
    }

    /// Finalize a transaction with a Simplicity witness for the specified input.
    #[allow(clippy::too_many_arguments)]
    pub fn finalize_transaction(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<Arc<TxOut>>,
        input_index: u32,
        witness_values: &SimplicityWitnessValues,
        network: &Network,
        log_level: SimplicityLogLevel,
    ) -> Result<Arc<Transaction>, LwkError> {
        let x_only_key = program_public_key.to_simplicityhl()?;
        let utxos_inner = convert_utxos(&utxos);

        let finalized = signer::finalize_transaction(
            tx.as_ref().clone(),
            &self.inner,
            &x_only_key,
            &utxos_inner,
            input_index as usize,
            witness_values.to_inner(),
            network.into(),
            log_level.into(),
        )?;

        Ok(Arc::new(finalized.into()))
    }

    /// Create a Schnorr signature for a P2PK Simplicity program input.
    #[allow(clippy::too_many_arguments)]
    pub fn create_p2pk_signature(
        &self,
        signer: &crate::Signer,
        derivation_path: String,
        tx: &Transaction,
        utxos: Vec<Arc<TxOut>>,
        input_index: u32,
        network: &Network,
    ) -> Result<Hex, LwkError> {
        let keypair = derive_keypair(signer, &derivation_path)?;
        let x_only_pubkey = keypair.x_only_public_key().0;
        let utxos_inner = convert_utxos(&utxos);

        let sighash = signer::get_sighash_all(
            tx.as_ref(),
            &self.inner,
            &x_only_pubkey,
            &utxos_inner,
            input_index as usize,
            network.into(),
        )?;

        let signature = keypair.sign_schnorr(sighash);

        Ok(Hex::from(signature.serialize().to_vec()))
    }

    /// Satisfy and execute this program in a transaction environment.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        tx: &Transaction,
        program_public_key: &XOnlyPublicKey,
        utxos: Vec<Arc<TxOut>>,
        input_index: u32,
        witness_values: &SimplicityWitnessValues,
        network: &Network,
        log_level: SimplicityLogLevel,
    ) -> Result<Arc<SimplicityRunResult>, LwkError> {
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
            witness_values.to_inner(),
            &env,
            log_level.into(),
        )?;

        Ok(Arc::new(SimplicityRunResult { pruned, value }))
    }
}
