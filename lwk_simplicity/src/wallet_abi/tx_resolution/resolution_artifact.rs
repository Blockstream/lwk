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
    wallet_input_indices: Vec<u32>,
    wallet_input_finalization_weight: usize,
}

impl ResolutionArtifacts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn secrets(&self) -> &HashMap<usize, TxOutSecrets> {
        &self.secrets
    }

    pub(crate) fn finalizers(&self) -> &[FinalizerSpec] {
        &self.finalizers
    }

    pub(crate) fn wallet_input_indices(&self) -> &[u32] {
        &self.wallet_input_indices
    }

    pub(crate) fn wallet_input_finalization_weight(&self) -> usize {
        self.wallet_input_finalization_weight
    }

    pub(crate) fn collect_wallet_input(
        &mut self,
        selected_wallet_utxo: &ExternalUtxo,
        input_index: usize,
    ) -> Result<(), WalletAbiError> {
        self.secrets
            .insert(input_index, selected_wallet_utxo.unblinded);
        self.finalizers.push(FinalizerSpec::Wallet);
        self.wallet_input_indices.push(u32::try_from(input_index)?);
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

    pub(crate) fn collect_input(
        &mut self,
        input: &InputSchema,
        input_index: usize,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        self.secrets.insert(input_index, *material.secrets());
        self.finalizers.push(input.finalizer.clone());

        if matches!(input.finalizer, FinalizerSpec::Wallet) {
            self.wallet_input_indices.push(u32::try_from(input_index)?);
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
