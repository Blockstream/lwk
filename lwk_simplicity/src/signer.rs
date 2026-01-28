use crate::error::ProgramError;
use crate::runner::run_program;
use crate::scripts::{control_block, create_p2tr_address};

use std::sync::Arc;

use simplicityhl::elements::secp256k1_zkp::Message;
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::simplicity::elements::{AddressParams, Transaction, TxInWitness, TxOut};
use simplicityhl::simplicity::hashes::Hash as _;
use simplicityhl::simplicity::jet::elements::{ElementsEnv, ElementsUtxo};
use simplicityhl::tracker::TrackerLogLevel;
use simplicityhl::{elements, CompiledProgram, WitnessValues};

/// Compute the sighash_all for signing a Simplicity program input.
///
/// This function returns the message that needs to be signed with a Schnorr signature.
/// The caller is responsible for signing the message with the appropriate key.
///
/// # Errors
///
/// Returns error if environment verification fails.
pub fn get_sighash_all(
    tx: &Transaction,
    program: &CompiledProgram,
    program_public_key: &XOnlyPublicKey,
    utxos: &[TxOut],
    input_index: usize,
    params: &'static AddressParams,
    genesis_hash: elements::BlockHash,
) -> Result<Message, ProgramError> {
    let env = get_and_verify_env(
        tx,
        program,
        program_public_key,
        utxos,
        params,
        genesis_hash,
        input_index,
    )?;

    let sighash_all = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

    Ok(sighash_all)
}

/// Finalize transaction with a Simplicity witness for the specified input.
///
/// # Errors
/// Returns error if environment verification or program execution fails.
#[allow(clippy::too_many_arguments)]
pub fn finalize_transaction(
    mut tx: Transaction,
    program: &CompiledProgram,
    program_public_key: &XOnlyPublicKey,
    utxos: &[TxOut],
    input_index: usize,
    witness_values: WitnessValues,
    params: &'static AddressParams,
    genesis_hash: elements::BlockHash,
    log_level: TrackerLogLevel,
) -> Result<Transaction, ProgramError> {
    let env = get_and_verify_env(
        &tx,
        program,
        program_public_key,
        utxos,
        params,
        genesis_hash,
        input_index,
    )?;

    let pruned = run_program(program, witness_values, &env, log_level)?.0;

    let (simplicity_program_bytes, simplicity_witness_bytes) = pruned.to_vec_with_witness();
    let cmr = pruned.cmr();

    tx.input[input_index].witness = TxInWitness {
        amount_rangeproof: None,
        inflation_keys_rangeproof: None,
        script_witness: vec![
            simplicity_witness_bytes,
            simplicity_program_bytes,
            cmr.as_ref().to_vec(),
            control_block(cmr, *program_public_key).serialize(),
        ],
        pegin_witness: vec![],
    };

    Ok(tx)
}

/// Build and verify an Elements environment for program execution.
///
/// # Errors
/// Returns error if UTXO index is invalid or script pubkey doesn't match.
pub fn get_and_verify_env(
    tx: &Transaction,
    program: &CompiledProgram,
    program_public_key: &XOnlyPublicKey,
    utxos: &[TxOut],
    params: &'static AddressParams,
    genesis_hash: elements::BlockHash,
    input_index: usize,
) -> Result<ElementsEnv<Arc<Transaction>>, ProgramError> {
    let cmr = program.commit().cmr();

    if utxos.len() <= input_index {
        return Err(ProgramError::UtxoIndexOutOfBounds {
            input_index,
            utxo_count: utxos.len(),
        });
    }

    let target_utxo = &utxos[input_index];
    let script_pubkey = create_p2tr_address(cmr, program_public_key, params).script_pubkey();

    if target_utxo.script_pubkey != script_pubkey {
        return Err(ProgramError::ScriptPubkeyMismatch {
            expected_hash: script_pubkey.script_hash().to_string(),
            actual_hash: target_utxo.script_pubkey.script_hash().to_string(),
        });
    }

    Ok(ElementsEnv::new(
        Arc::new(tx.clone()),
        utxos
            .iter()
            .map(|utxo| ElementsUtxo {
                script_pubkey: utxo.script_pubkey.clone(),
                asset: utxo.asset,
                value: utxo.value,
            })
            .collect(),
        u32::try_from(input_index)?,
        cmr,
        control_block(cmr, *program_public_key),
        None,
        genesis_hash,
    ))
}
