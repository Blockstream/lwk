//! Runtime transaction builder/finalizer.
//!
//! High-level flow:
//! 1. Build a fee-targeted PSET (`resolve_inputs` + `balance_out`).
//! 2. Estimate required fee from a finalized+blinded estimation transaction.
//! 3. Iterate fee target to fixed-point convergence (bounded).
//! 4. Build final PSET with converged fee, blind, finalize, and verify proofs.
//!
//! Fee-rate units:
//! - request field `fee_rate_sat_vb` is interpreted as sat/kvB
//! - value is passed directly to `lwk_common::calculate_fee`
//!
//! Fee convergence:
//! - initial target: `1 sat`
//! - max iterations: `MAX_FEE_ITERS`
//! - cycle handling: if oscillation is detected, escalate once to max cycle value
//! - failure mode: deterministic `Funding` error when convergence is not reached
//!
//! Formal references:
//! - Bitcoin Core coin selection context:
//!   <https://github.com/bitcoin/bitcoin/blob/master/src/wallet/coinselection.cpp>
//! - Murch, *An Evaluation of Coin Selection Strategies*:
//!   <http://murch.one/wp-content/uploads/2016/11/erhardt2016coinselection.pdf>
//!

use crate::error::WalletAbiError;
use crate::runner::run_program;
use crate::scripts::{control_block, load_program};
use crate::signer::get_and_verify_env;
use crate::wallet_abi::schema::runtime_deps::{SignerMeta, WalletMeta};
use crate::wallet_abi::schema::{
    resolve_arguments, resolve_witness, FinalizerSpec, TransactionInfo, TxCreateRequest,
    TxCreateResponse,
};
use crate::wallet_abi::tx_resolution::input_resolution::ResolutionState;
use crate::wallet_abi::tx_resolution::output_resolution::balance_out;
use crate::wallet_abi::tx_resolution::utils::{
    get_finalizer_spec_key, get_secrets_spec_key, DEFAULT_FEE_RATE_SAT_KVB, MAX_FEE_ITERS,
};

use std::collections::HashMap;

use lwk_common::calculate_fee;
use lwk_wollet::elements::hex::ToHex;
use lwk_wollet::elements::pset::serialize::Serialize;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{BlockHash, Transaction, TxOut, TxOutSecrets};
use lwk_wollet::elements_miniscript::psbt::PsbtExt;
use lwk_wollet::hashes::Hash;
use lwk_wollet::secp256k1::rand::thread_rng;
use lwk_wollet::WalletTxOut;
use lwk_wollet::EC;

use simplicityhl::tracker::TrackerLogLevel;

use log::error;

pub struct Runtime<'a, Signer: SignerMeta, Wallet: WalletMeta> {
    request: TxCreateRequest,
    signer_meta: &'a Signer,
    wallet_meta: &'a Wallet,
}

impl<'a, Signer: SignerMeta, Wallet: WalletMeta> Runtime<'a, Signer, Wallet>
where
    WalletAbiError: From<Signer::Error> + From<Wallet::Error>,
{
    pub fn build(
        request: TxCreateRequest,
        signer_meta: &'a Signer,
        wallet_meta: &'a Wallet,
    ) -> Self {
        Self {
            request,
            signer_meta,
            wallet_meta,
        }
    }

    pub async fn process_request(&self) -> Result<TxCreateResponse, WalletAbiError> {
        self.request
            .validate_for_runtime(self.signer_meta.get_network())?;

        let fee_rate_sat_kvb = self
            .request
            .params
            .fee_rate_sat_kvb
            .unwrap_or(DEFAULT_FEE_RATE_SAT_KVB);
        let fee_rate_sat_kvb = validate_fee_rate_sat_kvb(fee_rate_sat_kvb)?;

        // Freeze wallet snapshot once per request so fee-convergence iterations stay
        // deterministic with respect to wallet candidate pool.
        let wallet_snapshot = self.wallet_meta.get_spendable_utxos().await?;
        let finalized_tx = self.finalize(fee_rate_sat_kvb, &wallet_snapshot).await?;

        let txid = finalized_tx.txid();

        if self.request.broadcast {
            let published_txid = self
                .wallet_meta
                .broadcast_transaction(finalized_tx.clone())
                .await?;
            if txid != published_txid {
                error!("broadcast txid mismatch: locally built txid={txid}, esplora returned txid={published_txid}");

                return Err(WalletAbiError::InvalidResponse(
                    "broadcast txid mismatch".to_string(),
                ));
            }
        }

        let response = TxCreateResponse::ok(
            &self.request,
            TransactionInfo {
                tx_hex: finalized_tx.serialize().to_hex(),
                txid,
            },
            None,
        );

        Ok(response)
    }

    /// Build, blind and finalize a transaction with bounded fee fixed-point convergence.
    ///
    /// The output stage models fee as explicit policy-asset demand, so this method iterates
    /// `fee_target_sat` until the estimated fee matches the target.
    ///
    /// Wallet snapshot determinism:
    /// - all fee iterations and the final build share one preloaded wallet snapshot
    /// - runtime does not refresh candidate UTXOs during the loop
    ///
    /// Failure conditions:
    /// - convergence not reached within `MAX_FEE_ITERS`
    /// - any intermediate funding deficit raised by resolvers
    async fn finalize(
        &self,
        fee_rate_sat_kvb: f32,
        wallet_snapshot: &[WalletTxOut],
    ) -> Result<Transaction, WalletAbiError> {
        // Bounded fixed-point fee convergence:
        // fee_target -> build tx -> estimate fee -> repeat until stable or cap reached.
        let mut fee_target_sat = 1u64;
        let mut seen_targets = Vec::new();
        let mut escalated_cycle_once = false;
        let mut converged_fee_target = None;

        for _ in 0..MAX_FEE_ITERS {
            let estimated_fee_sat = self
                .estimate_fee_target(fee_target_sat, fee_rate_sat_kvb, wallet_snapshot)
                .await?;

            if estimated_fee_sat == fee_target_sat {
                converged_fee_target = Some(estimated_fee_sat);
                break;
            }

            if let Some(cycle_start) = seen_targets
                .iter()
                .position(|previous| *previous == estimated_fee_sat)
            {
                let cycle_max = seen_targets[cycle_start..]
                    .iter()
                    .copied()
                    .chain(std::iter::once(estimated_fee_sat))
                    .max()
                    .unwrap_or(estimated_fee_sat);
                if !escalated_cycle_once {
                    escalated_cycle_once = true;
                    seen_targets.push(fee_target_sat);
                    fee_target_sat = cycle_max;
                    continue;
                }
            }

            seen_targets.push(fee_target_sat);
            fee_target_sat = estimated_fee_sat;
        }

        let converged_fee_target = converged_fee_target.ok_or_else(|| {
            let visited_targets = seen_targets
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");

            error!("fee convergence failed after {MAX_FEE_ITERS} iterations; last target={fee_target_sat} sat, visited=[{visited_targets}]");

            WalletAbiError::Funding("fee convergence failed".to_string())
        })?;

        let mut pst = self
            .build_transaction(converged_fee_target, wallet_snapshot)
            .await?;
        let inp_txout_secrets = input_blinding_secrets(&pst)?;
        pst.blind_last(&mut thread_rng(), &EC, &inp_txout_secrets)?;
        let pst = self.finalize_all_inputs(pst)?;

        let utxos: Vec<TxOut> = pst
            .inputs()
            .iter()
            .filter_map(|x| x.witness_utxo.clone())
            .collect();

        let tx = pst.extract_tx()?;

        // `elements::Transaction::verify_tx_amt_proofs` treats zero-value OP_RETURN outputs
        // as a hard error even though Elements accepts them as provably unspendable. Lending
        // contracts use these outputs for metadata and burns, so skip the local proof check
        // for that specific transaction shape and rely on node validation instead.
        if !tx.output.iter().any(|tx_out| {
            tx_out.script_pubkey.is_provably_unspendable() && tx_out.value.explicit() == Some(0)
        }) {
            tx.verify_tx_amt_proofs(&EC, &utxos)?;
        }

        Ok(tx)
    }

    /// Estimate required fee for a candidate fee target using a finalized+blinded estimation tx.
    ///
    /// This is used inside the bounded fixed-point loop in `finalize`.
    async fn estimate_fee_target(
        &self,
        fee_target_sat: u64,
        fee_rate_sat_kvb: f32,
        wallet_snapshot: &[WalletTxOut],
    ) -> Result<u64, WalletAbiError> {
        let fee_estimation_build = self
            .build_transaction(fee_target_sat, wallet_snapshot)
            .await?;
        let mut pst = fee_estimation_build;
        let inp_txout_secrets = input_blinding_secrets(&pst)?;
        pst.blind_last(&mut thread_rng(), &EC, &inp_txout_secrets)?;
        let pst = self.finalize_all_inputs(pst)?;

        Ok(calculate_fee(
            pst.extract_tx()?.discount_weight(),
            fee_rate_sat_kvb,
        ))
    }

    /// Build a fee-targeted PSET by running fee-aware input and output resolvers.
    async fn build_transaction(
        &self,
        fee_target_sat: u64,
        wallet_snapshot: &[WalletTxOut],
    ) -> Result<PartiallySignedTransaction, WalletAbiError> {
        let mut resolver =
            ResolutionState::build(self.signer_meta, self.wallet_meta, wallet_snapshot.to_vec())?;

        let mut pst = PartiallySignedTransaction::new_v2();
        pst.global.tx_data.fallback_locktime = self.request.params.lock_time;

        pst = resolver
            .resolve_inputs(pst, &self.request.params, fee_target_sat)
            .await?;

        pst = balance_out(self.signer_meta, pst, &self.request.params, fee_target_sat)?;

        Ok(pst)
    }

    pub fn finalize_all_inputs(
        &self,
        mut pst: PartiallySignedTransaction,
    ) -> Result<PartiallySignedTransaction, WalletAbiError> {
        let env_utxos = execution_env_utxos(&pst)?;

        self.signer_meta.sign_pst(&mut pst)?;

        for input_index in 0..pst.inputs().len() {
            let finalizer = input_finalizer_spec(&pst, input_index)?;

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
                FinalizerSpec::Simf {
                    source_simf,
                    internal_key,
                    arguments,
                    witness,
                } => {
                    let arguments = resolve_arguments(&arguments, &pst)?;

                    let program = load_program(&source_simf, arguments)?;

                    let env = get_and_verify_env(
                        &pst.extract_tx()?,
                        &program,
                        &internal_key.get_x_only_pubkey(),
                        &env_utxos,
                        self.signer_meta.get_network(),
                        input_index,
                    )?;

                    let witness = resolve_witness(&witness, self.signer_meta, &env)?;

                    let (pruned, _) = run_program(&program, witness, &env, TrackerLogLevel::Trace)?;

                    let (simplicity_program_bytes, simplicity_witness_bytes) =
                        pruned.to_vec_with_witness();
                    let cmr = pruned.cmr();

                    pst.inputs_mut()[input_index].final_script_witness = Some(vec![
                        simplicity_witness_bytes,
                        simplicity_program_bytes,
                        cmr.as_ref().to_vec(),
                        control_block(cmr, internal_key.get_x_only_pubkey()).serialize(),
                    ]);
                }
            }
        }

        Ok(pst)
    }
}

/// Build UTXOs used by Simplicity execution environment.
///
/// Keep environment UTXOs identical to the transaction witness UTXOs so jets such
/// as `SigAllHash` are computed against the same prevout representation the network
/// validates.
fn execution_env_utxos(pst: &PartiallySignedTransaction) -> Result<Vec<TxOut>, WalletAbiError> {
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

/// Collect input secrets used by blinding and surjection-proof domain construction.
fn input_blinding_secrets(
    pst: &PartiallySignedTransaction,
) -> Result<HashMap<usize, TxOutSecrets>, WalletAbiError> {
    let mut inp_txout_secrets: HashMap<usize, TxOutSecrets> = HashMap::new();
    for (input_index, input) in pst.inputs().iter().enumerate() {
        let encoded_secrets = input
            .proprietary
            .get(&get_secrets_spec_key())
            .ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "missing input blinding secrets metadata for input index {input_index}"
                ))
            })?;
        let secrets: TxOutSecrets = serde_json::from_slice(encoded_secrets)?;
        inp_txout_secrets.insert(input_index, secrets);
    }

    Ok(inp_txout_secrets)
}

fn input_finalizer_spec(
    pst: &PartiallySignedTransaction,
    input_index: usize,
) -> Result<FinalizerSpec, WalletAbiError> {
    let finalizer_payload = pst
        .inputs()
        .get(input_index)
        .ok_or_else(|| {
            WalletAbiError::InvalidRequest(format!(
                "missing input index {input_index} while finalizing transaction"
            ))
        })?
        .proprietary
        .get(&get_finalizer_spec_key())
        .ok_or_else(|| {
            WalletAbiError::InvalidRequest(format!(
                "missing finalizer metadata for input index {input_index}"
            ))
        })?;

    FinalizerSpec::decode(finalizer_payload)
}

fn validate_fee_rate_sat_kvb(fee_rate_sat_kvb: f32) -> Result<f32, WalletAbiError> {
    if !fee_rate_sat_kvb.is_finite() {
        return Err(WalletAbiError::InvalidRequest(format!(
            "invalid fee rate (sat/kvB): expected finite value, got {fee_rate_sat_kvb}"
        )));
    }
    if fee_rate_sat_kvb < 0.0 {
        return Err(WalletAbiError::InvalidRequest(format!(
            "invalid fee rate (sat/kvB): expected non-negative value, got {fee_rate_sat_kvb}"
        )));
    }

    Ok(fee_rate_sat_kvb)
}
