use crate::error::WalletAbiError;
use crate::runner::run_program;
use crate::scripts::{control_block, load_program};
use crate::signer::get_and_verify_env;
use crate::wallet_abi::schema::{resolve_arguments, resolve_witness, FinalizerSpec, KeyStoreMeta};

use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{BlockHash, TxOut};
use lwk_wollet::elements_miniscript::psbt::PsbtExt;
use lwk_wollet::hashes::Hash;
use lwk_wollet::EC;

use simplicityhl::tracker::TrackerLogLevel;

/// Finalize wallet-owned inputs after signing so standard wallet witnesses are
/// materialized without touching Simplicity inputs, which need a separate
/// environment-driven witness construction pass.
pub(crate) fn finalize_wallet_inputs<Signer>(
    signer_meta: &Signer,
    mut pst: PartiallySignedTransaction,
    finalizers: &[FinalizerSpec],
) -> Result<PartiallySignedTransaction, WalletAbiError>
where
    Signer: KeyStoreMeta,
    WalletAbiError: From<Signer::Error>,
{
    signer_meta.sign_pst(&mut pst)?;

    for input_index in 0..pst.inputs().len() {
        let finalizer = finalizers.get(input_index).ok_or_else(|| {
            WalletAbiError::InvalidRequest(format!(
                "missing input finalizer metadata for input index {input_index}"
            ))
        })?;

        match finalizer {
            FinalizerSpec::Wallet => {
                // Wallet-signature path currently uses non-taproot miniscript finalization.
                // `BlockHash::all_zeros()` is expected for this flow.
                pst.finalize_inp_mut(&EC, input_index, BlockHash::all_zeros())
                    .map_err(|error| {
                        WalletAbiError::InvalidFinalizationSteps(format!(
                            "wallet finalization failed for input index {input_index}: {error}"
                        ))
                    })?;
            }
            FinalizerSpec::Simf { .. } => continue,
        }
    }

    Ok(pst)
}

// FIXME: currently we are explicitly relying on the fact that only 2 possible finalizers exist to avoid overcomplicating fee estimation.
/// Finalize Simplicity inputs in their own pass because their witness is
/// derived from the blinded transaction, resolved environment UTXOs, and
/// executed program rather than the wallet miniscript finalizer.
pub(crate) fn finalize_simf_inputs<Signer>(
    signer_meta: &Signer,
    mut pst: PartiallySignedTransaction,
    finalizers: &[FinalizerSpec],
    network: lwk_common::Network,
) -> Result<PartiallySignedTransaction, WalletAbiError>
where
    Signer: KeyStoreMeta,
    WalletAbiError: From<Signer::Error>,
{
    let env_utxos = extract_env_utxos(&pst)?;

    for input_index in 0..pst.inputs().len() {
        let finalizer = finalizers.get(input_index).ok_or_else(|| {
            WalletAbiError::InvalidRequest(format!(
                "missing input finalizer metadata for input index {input_index}"
            ))
        })?;

        match finalizer {
            FinalizerSpec::Wallet => continue,
            FinalizerSpec::Simf {
                source_simf,
                internal_key,
                arguments,
                witness,
            } => {
                let arguments = resolve_arguments(arguments, &pst)?;

                let program = load_program(source_simf, arguments)?;

                let env = get_and_verify_env(
                    &pst.extract_tx()?,
                    &program,
                    &internal_key.get_x_only_pubkey(),
                    &env_utxos,
                    network,
                    input_index,
                )?;

                let witness = resolve_witness(witness, signer_meta, &env)?;

                let (pruned, _) = run_program(&program, witness, &env, TrackerLogLevel::Trace)?;

                let (simplicity_program_bytes, simplicity_witness_bytes) =
                    pruned.to_vec_with_witness();
                let cmr = pruned.cmr();

                pst.inputs_mut()[input_index].final_script_witness = Some(vec![
                    simplicity_witness_bytes,
                    simplicity_program_bytes,
                    cmr.as_ref().to_vec(),
                    // TODO: add an ability to use TaprootSpendInfo
                    control_block(cmr, internal_key.get_x_only_pubkey()).serialize(),
                ]);
            }
        }
    }

    Ok(pst)
}

/// Build UTXOs used by Simplicity execution environment.
///
/// Keep environment UTXOs identical to the transaction witness UTXOs so jets such
/// as `SigAllHash` are computed against the same prevout representation the network
/// validates.
pub(crate) fn extract_env_utxos(
    pst: &PartiallySignedTransaction,
) -> Result<Vec<TxOut>, WalletAbiError> {
    let mut utxos = Vec::with_capacity(pst.inputs().len());

    for (input_index, input) in pst.inputs().iter().enumerate() {
        let witness_utxo = input.witness_utxo.clone().ok_or_else(|| {
            WalletAbiError::InvalidRequest(format!(
                "missing witness_utxo for input index {input_index} while building simplicity env"
            ))
        })?;
        utxos.push(witness_utxo);
    }

    Ok(utxos)
}
