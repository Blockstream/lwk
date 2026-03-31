use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{FinalizerSpec, InputSchema};
use crate::wallet_abi::tx_resolution::input_material::ResolvedInputMaterial;

use std::collections::HashMap;

use lwk_wollet::elements::{Script, TxOutSecrets};
use lwk_wollet::ExternalUtxo;

#[derive(Debug, Default)]
pub(crate) struct ResolutionArtifacts {
    secrets: HashMap<usize, TxOutSecrets>,
    finalizers: Vec<FinalizerSpec>,
    wallet_input_finalization_weight: usize,
}

impl ResolutionArtifacts {
    /// Create empty artifacts so resolution can accumulate secrets, finalizers,
    /// and fee-model inputs in PSET order.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Expose collected secrets because blinding must reference the same
    /// per-input unblinding data captured during resolution.
    pub(crate) fn secrets(&self) -> &HashMap<usize, TxOutSecrets> {
        &self.secrets
    }

    /// Expose finalizer metadata so later finalization passes can mirror the
    /// resolved input ordering without re-reading request state.
    pub(crate) fn finalizers(&self) -> &[FinalizerSpec] {
        &self.finalizers
    }

    /// Expose accumulated wallet finalization weight so fee estimation can
    /// account for wallet witnesses before they are materialized.
    pub(crate) fn wallet_input_finalization_weight(&self) -> usize {
        self.wallet_input_finalization_weight
    }

    /// Record one auxiliary wallet input in artifact state so blinding,
    /// finalization, and fee modeling all stay aligned with the concrete PSET
    /// input index that was added.
    pub(crate) fn collect_wallet_input(
        &mut self,
        selected_wallet_utxo: &ExternalUtxo,
        input_index: usize,
    ) -> Result<(), WalletAbiError> {
        self.secrets
            .insert(input_index, selected_wallet_utxo.unblinded);
        self.finalizers.push(FinalizerSpec::Wallet);
        self.wallet_input_finalization_weight = self
            .wallet_input_finalization_weight
            .checked_add(selected_wallet_utxo.max_weight_to_satisfy)
            .ok_or_else(|| {
                WalletAbiError::InvalidRequest(
                    "wallet input finalization weight overflow".to_string(),
                )
            })?;

        Ok(())
    }

    /// Capture artifacts for one resolved declared input so later stages do not
    /// need to re-resolve secrets, finalizer choice, or fee-model metadata.
    pub(crate) fn collect_input(
        &mut self,
        input: &InputSchema,
        input_index: usize,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        self.secrets.insert(input_index, *material.secrets());
        self.finalizers.push(input.finalizer.clone());

        if matches!(input.finalizer, FinalizerSpec::Wallet) {
            let modeled_weight = match *material.wallet_finalization_weight() {
                Some(weight) => weight,
                None => modeled_wallet_input_finalization_weight(
                    input_index,
                    &material.tx_out().script_pubkey,
                )?,
            };
            self.wallet_input_finalization_weight = self
                .wallet_input_finalization_weight
                .checked_add(modeled_weight)
                .ok_or_else(|| {
                    WalletAbiError::InvalidRequest(
                        "wallet input finalization weight overflow".to_string(),
                    )
                })?;
        }

        Ok(())
    }
}

/// Model wallet witness weight only when the wallet snapshot did not provide
/// one, allowing fee estimation to succeed for simple wallet inputs.
fn modeled_wallet_input_finalization_weight(
    input_index: usize,
    script_pubkey: &Script,
) -> Result<usize, WalletAbiError> {
    if script_pubkey.is_v0_p2wpkh() {
        // P2WPKH witness serialization:
        // - stack item count
        // - DER signature plus sighash byte
        // - compressed public key
        return Ok(1 + 1 + 73 + 1 + 33);
    }

    Err(WalletAbiError::InvalidRequest(format!(
        "unsupported wallet input script kind '{}' for fee estimation at input index {input_index}; provide a wallet snapshot entry with modeled weight or use p2wpkh",
        wallet_input_script_kind(script_pubkey)
    )))
}

/// Convert script templates into stable labels so unsupported wallet input kinds
/// produce clearer fee-estimation errors.
fn wallet_input_script_kind(script_pubkey: &Script) -> &'static str {
    if script_pubkey.is_v0_p2wpkh() {
        "p2wpkh"
    } else if script_pubkey.is_v0_p2wsh() {
        "p2wsh"
    } else if script_pubkey.is_v1_p2tr() {
        "p2tr"
    } else if script_pubkey.is_p2sh() {
        "p2sh"
    } else if script_pubkey.is_p2pkh() {
        "p2pkh"
    } else if script_pubkey.is_p2pk() {
        "p2pk"
    } else {
        "unknown"
    }
}
