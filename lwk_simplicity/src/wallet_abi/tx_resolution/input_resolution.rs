//! Input resolution for transaction construction.
//!
//! This module balances the input/output equation in four phases:
//! 1. Build demand from all requested outputs and inject implicit fee demand on policy asset.
//! 2. Resolve declared inputs in order.
//! 3. Materialize deferred issuance-linked output demand when referenced inputs are known.
//! 4. Add auxiliary wallet inputs until every positive asset deficit is closed.
//!
//! # Algorithm
//!
//! Auxiliary funding for each asset deficit uses a deterministic stack:
//! 1. Bounded Branch-and-Bound (`BnB`) for exact subset match.
//! 2. Deterministic single-input fallback (largest UTXO above target).
//! 3. Deterministic largest-first accumulation fallback.
//!
//! This mirrors formal coin-selection framing (subset-sum / knapsack) while keeping runtime
//! bounded by an explicit node cap.
//!
//! # Determinism
//!
//! Candidate order and tie-breaks are stable:
//! - primary sort: amount descending
//! - tie-break 1: `txid` lexicographic ascending
//! - tie-break 2: `vout` ascending
//!
//! For multiple exact `BnB` matches with equal input count, the lexicographically smaller
//! outpoint list is selected.
//!
//! # Complexity
//!
//! Let:
//! - `O` = number of outputs
//! - `I` = number of declared inputs
//! - `U` = wallet UTXO count in snapshot
//! - `A` = number of distinct demanded assets
//! - `K` = number of auxiliary inputs added
//! - `N` = max candidate UTXOs for one deficit asset
//!
//! Worst-case time is:
//! - declared-input selection: `O(I * U * A)`
//! - auxiliary selection per deficit asset: bounded Branch-and-Bound search
//!   plus deterministic fallbacks, `O(MAX_BNB_NODES + N)`
//! - overall: `O(I * U * A + K * (MAX_BNB_NODES + N) + O + I)`
//!
//! Space is `O(U + A + O + N)` for used-outpoint tracking, equation state and
//! per-asset candidate working sets.
//!
//! # Failure modes
//!
//! - Arithmetic overflow fails with `InvalidRequest`.
//! - Unclosable deficits fail with `Funding`.

use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{
    AmountFilter, AssetFilter, AssetVariant, FinalizerSpec, InputIssuance, InputIssuanceKind,
    InputSchema, InputUnblinding, LockFilter, RuntimeParams, UTXOSource, WalletSourceFilter,
};
use crate::wallet_abi::tx_resolution::{get_finalizer_spec_key, get_secrets_spec_key};

use crate::wallet_abi::schema::runtime_deps::{SignerMeta, WalletMeta};
use crate::wallet_abi::tx_resolution::bnb::{
    bnb_exact_subset_indices, select_largest_first_accumulation,
    select_single_largest_above_target, sum_selected_amount, BnbCandidate,
};

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};

use lwk_common::Bip::Bip84;

use lwk_wollet::bitcoin::bip32::{ChildNumber, DerivationPath};
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::elements::confidential::{Asset, AssetBlindingFactor, Value, ValueBlindingFactor};
use lwk_wollet::elements::hashes::sha256::Midstate;
use lwk_wollet::elements::hashes::Hash;
use lwk_wollet::elements::pset::{Input, PartiallySignedTransaction};
use lwk_wollet::elements::{secp256k1_zkp, AssetId, ContractHash, OutPoint, TxOut, TxOutSecrets};
use lwk_wollet::secp256k1::constants::ONE;
use lwk_wollet::{Chain, WalletTxOut, EC};

type CandidateScore = (u64, u64, u64, String, u32);

#[derive(Clone, Copy)]
struct WalletDerivationIndex {
    ext_int: Chain,
    wildcard_index: u32,
}

#[derive(Clone, Copy)]
enum DeferredDemandKind {
    NewIssuanceAsset,
    NewIssuanceToken,
    ReIssuanceAsset,
}

pub struct ResolutionState<'a, Signer: SignerMeta, Wallet: WalletMeta> {
    signer_meta: &'a Signer,
    wallet_meta: &'a Wallet,
    wallet_snapshot: Vec<WalletTxOut>,
    used_outpoints: HashSet<OutPoint>,
    demand_by_asset: BTreeMap<AssetId, u64>,
    supply_by_asset: BTreeMap<AssetId, u64>,
    deferred_demands: HashMap<u32, Vec<(DeferredDemandKind, u64)>>,
}

struct ResolvedInputMaterial {
    outpoint: OutPoint,
    tx_out: TxOut,
    secrets: TxOutSecrets,
    wallet_derivation: Option<WalletDerivationIndex>,
}

impl<'a, Signer: SignerMeta, Wallet: WalletMeta> ResolutionState<'a, Signer, Wallet>
where
    WalletAbiError: From<Signer::Error> + From<Wallet::Error>,
{
    pub fn build(
        signer_meta: &'a Signer,
        wallet_meta: &'a Wallet,
        wallet_snapshot: Vec<WalletTxOut>,
    ) -> Result<Self, WalletAbiError> {
        if let Some(spent) = wallet_snapshot.iter().find(|utxo| utxo.is_spent) {
            return Err(WalletAbiError::InvalidResponse(format!(
                "wallet snapshot contains spent UTXO {}:{}; runtime requires unspent-only candidate set",
                spent.outpoint.txid, spent.outpoint.vout
            )));
        }

        // Phase 1: initialize demand/supply state using a preloaded snapshot.
        // We keep all equation state in a dedicated struct so each phase mutates a single object.
        Ok(Self {
            signer_meta,
            wallet_meta,
            wallet_snapshot,
            used_outpoints: Default::default(),
            demand_by_asset: Default::default(),
            supply_by_asset: Default::default(),
            deferred_demands: Default::default(),
        })
    }

    /// Resolve all inputs required to satisfy output demand, including issuance-derived demand.
    ///
    /// The algorithm first consumes declared inputs, then greedily appends auxiliary wallet
    /// inputs until the equation has no positive deficits.
    ///
    /// Fee nuance:
    /// - Fee demand is injected implicitly as policy-asset demand equal to `fee_target_sat`.
    ///
    /// Change nuance:
    /// - This resolver does not create or place change outputs.
    /// - It guarantees only `supply >= demand` per asset after resolution.
    /// - Any surplus created by UTXO granularity/overshoot is left for the output stage
    ///   to materialize as explicit change.
    ///
    /// # Complexity
    ///
    /// Let `I` be declared inputs, `U` wallet UTXOs, `A` demanded assets, and `K` auxiliary
    /// inputs added. Declared-input selection is `O(I * U * A)`. Auxiliary per-asset funding
    /// is bounded by `MAX_BNB_NODES` search plus deterministic fallbacks.
    pub async fn resolve_inputs(
        &mut self,
        mut pst: PartiallySignedTransaction,
        params: &RuntimeParams,
        fee_target_sat: u64,
    ) -> Result<PartiallySignedTransaction, WalletAbiError> {
        // Phase 2: build output demand from AssetVariant.
        // AssetId contributes directly, while issuance-linked variants are deferred until their
        // referenced input is resolved and its issuance entropy is known.
        self.resolve_output_demands(params, fee_target_sat)?;

        // Phase 3: resolve declared inputs in order.
        // Each input updates the PSET, contributes supply, and may unlock deferred output demand.
        self.resolve_declared_inputs(&mut pst, params).await?;

        // Safety check: all deferred output demands must have been activated by now.
        if !self.deferred_demands.is_empty() {
            return Err(WalletAbiError::InvalidRequest(
                "unresolved deferred output demands remain after input resolution".to_string(),
            ));
        }

        // Phase 4: if the declared inputs do not close the equation, add auxiliary wallet inputs
        // greedily by largest remaining deficit asset until fully balanced.
        self.add_auxiliary_inputs_until_balanced(&mut pst).await?;

        Ok(pst)
    }

    /// Build demand from output specs and store issuance-linked entries as deferred.
    ///
    /// Rules:
    /// - Non-fee outputs contribute demand directly (or deferred for issuance-derived assets).
    /// - Exactly one implicit policy-asset demand entry is added for `fee_target_sat`.
    fn resolve_output_demands(
        &mut self,
        params: &RuntimeParams,
        fee_target_sat: u64,
    ) -> Result<(), WalletAbiError> {
        let policy_asset = *self.signer_meta.get_network().policy_asset();

        // Convert output-level asset requirements into equation demand.
        // Issuance-derived outputs are deferred until their referenced input is resolved.
        for output in &params.outputs {
            match &output.asset {
                AssetVariant::AssetId { asset_id } => {
                    add_balance(&mut self.demand_by_asset, *asset_id, output.amount_sat)?;
                }
                AssetVariant::NewIssuanceAsset { input_index } => {
                    validate_output_input_index(&output.id, *input_index, params.inputs.len())?;
                    self.deferred_demands
                        .entry(*input_index)
                        .or_default()
                        .push((DeferredDemandKind::NewIssuanceAsset, output.amount_sat));
                }
                AssetVariant::NewIssuanceToken { input_index } => {
                    validate_output_input_index(&output.id, *input_index, params.inputs.len())?;
                    self.deferred_demands
                        .entry(*input_index)
                        .or_default()
                        .push((DeferredDemandKind::NewIssuanceToken, output.amount_sat));
                }
                AssetVariant::ReIssuanceAsset { input_index } => {
                    validate_output_input_index(&output.id, *input_index, params.inputs.len())?;
                    self.deferred_demands
                        .entry(*input_index)
                        .or_default()
                        .push((DeferredDemandKind::ReIssuanceAsset, output.amount_sat));
                }
            }
        }

        // Fee demand is always modeled from runtime target, independent of params fee amount.
        add_balance(&mut self.demand_by_asset, policy_asset, fee_target_sat)?;

        Ok(())
    }

    /// Resolve all declared inputs in order and mutate both PSET and equation state.
    async fn resolve_declared_inputs(
        &mut self,
        pst: &mut PartiallySignedTransaction,
        params: &RuntimeParams,
    ) -> Result<(), WalletAbiError> {
        // Main declared-input pass:
        // resolve source -> append PSET input -> increase supply -> unlock deferred demands.
        for (input_index, input) in params.inputs.iter().enumerate() {
            let material = self.resolve_declared_input_material(input).await?;

            self.add_resolved_input_to_pset(pst, input, &material)?;
            self.apply_input_supply(input, &material)?;
            self.activate_deferred_demands_for_input(input_index, input, &material)?;
        }

        Ok(())
    }

    /// Resolve one declared input from either provided or wallet source.
    async fn resolve_declared_input_material(
        &mut self,
        input: &InputSchema,
    ) -> Result<ResolvedInputMaterial, WalletAbiError> {
        match &input.utxo_source {
            UTXOSource::Wallet { filter } => {
                self.resolve_wallet_input_material(input, filter).await
            }
            UTXOSource::Provided { outpoint } => {
                self.resolve_provided_input_material(input, *outpoint).await
            }
        }
    }

    /// Resolve input material from wallet snapshot using deficit-aware selection.
    async fn resolve_wallet_input_material(
        &mut self,
        input: &InputSchema,
        filter: &WalletSourceFilter,
    ) -> Result<ResolvedInputMaterial, WalletAbiError> {
        if !matches!(input.unblinding, InputUnblinding::Wallet) {
            return Err(WalletAbiError::InvalidRequest(format!(
                "input '{}' uses utxo_source=wallet but unblinding is not 'wallet'",
                input.id
            )));
        }

        let selected = self.filter_tx_out(filter)?.ok_or_else(|| {
            WalletAbiError::Funding(format!(
                "no wallet UTXO matched contract input '{}' filter",
                input.id
            ))
        })?;

        self.reserve_outpoint(&input.id, selected.outpoint)?;

        let tx_out = self.wallet_meta.get_tx_out(selected.outpoint).await?;

        Ok(ResolvedInputMaterial {
            outpoint: selected.outpoint,
            tx_out,
            secrets: selected.unblinded,
            wallet_derivation: Some(WalletDerivationIndex {
                ext_int: selected.ext_int,
                wildcard_index: selected.wildcard_index,
            }),
        })
    }

    /// Resolve input material from a provided outpoint and optional unblinding hints.
    async fn resolve_provided_input_material(
        &mut self,
        input: &InputSchema,
        outpoint: OutPoint,
    ) -> Result<ResolvedInputMaterial, WalletAbiError> {
        self.reserve_outpoint(&input.id, outpoint)?;

        let tx_out = self.wallet_meta.get_tx_out(outpoint).await?;

        let secrets = match &input.unblinding {
            InputUnblinding::Wallet => self.signer_meta.unblind(&tx_out)?,
            InputUnblinding::Provided { secret_key } => {
                tx_out.unblind(&EC, *secret_key).map_err(|error| {
                    WalletAbiError::InvalidRequest(format!(
                        "unable to unblind input '{}' with provided unblinding key: {error}",
                        input.id
                    ))
                })?
            }
            InputUnblinding::Explicit => {
                let (Asset::Explicit(asset), Value::Explicit(value)) = (tx_out.asset, tx_out.value)
                else {
                    return Err(WalletAbiError::InvalidRequest(format!(
                        "marked input '{}' as explicit when the confidential was provided",
                        input.id
                    )));
                };

                TxOutSecrets {
                    asset,
                    asset_bf: AssetBlindingFactor::zero(),
                    value,
                    value_bf: ValueBlindingFactor::zero(),
                }
            }
        };

        Ok(ResolvedInputMaterial {
            outpoint,
            tx_out,
            secrets,
            wallet_derivation: None,
        })
    }

    /// Apply the resolved input contribution to equation supply (base + issuance minting).
    fn apply_input_supply(
        &mut self,
        input: &InputSchema,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        add_balance(
            &mut self.supply_by_asset,
            material.secrets.asset,
            material.secrets.value,
        )?;

        if let Some(issuance) = input.issuance.as_ref() {
            let issuance_entropy = calculate_issuance_entropy(material.outpoint, issuance);
            let issuance_asset = AssetId::from_entropy(issuance_entropy);
            add_balance(
                &mut self.supply_by_asset,
                issuance_asset,
                issuance.asset_amount_sat,
            )?;

            if issuance.token_amount_sat > 0 {
                let token_asset =
                    issuance_token_from_entropy_for_unblinded_issuance(issuance_entropy);
                add_balance(
                    &mut self.supply_by_asset,
                    token_asset,
                    issuance.token_amount_sat,
                )?;
            }
        }

        Ok(())
    }

    /// Append a resolved input to the PSET and attach sequence, prevout and witness UTXO.
    fn add_resolved_input_to_pset(
        &self,
        pst: &mut PartiallySignedTransaction,
        input: &InputSchema,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        let mut pset_input = Input::from_prevout(material.outpoint);
        pset_input.sequence = Some(input.sequence);
        pset_input.witness_utxo = Some(material.tx_out.clone());
        pset_input.amount = Some(material.secrets.value);
        pset_input.asset = Some(material.secrets.asset);

        if let Some(issuance) = input.issuance.as_ref() {
            apply_issuance_to_pset_input(&mut pset_input, issuance, material)?;
        }

        pset_input
            .proprietary
            .insert(get_finalizer_spec_key(), input.finalizer.try_encode()?);
        pset_input.proprietary.insert(
            get_secrets_spec_key(),
            serde_json::to_vec(&material.secrets)?,
        );
        if let Some(index) = material.wallet_derivation {
            let (pubkey, derivation_path) = self.signer_origin_for_wallet_utxo(index)?;
            pset_input
                .bip32_derivation
                .insert(pubkey, (self.signer_meta.fingerprint(), derivation_path));
        }
        pst.add_input(pset_input);

        Ok(())
    }

    /// Convert deferred issuance-linked demand into concrete asset demand for one input index.
    fn activate_deferred_demands_for_input(
        &mut self,
        input_index: usize,
        input: &InputSchema,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        // Deferred demands become concrete once the referenced input is known,
        // because issuance-derived asset ids depend on that input outpoint/entropy.
        let input_index_u32 = u32::try_from(input_index)?;
        let Some(entries) = self.deferred_demands.remove(&input_index_u32) else {
            return Ok(());
        };

        let issuance = input.issuance.as_ref().ok_or_else(|| {
            WalletAbiError::InvalidRequest(format!(
                "output asset references input {} but input '{}' has no issuance metadata",
                input_index, input.id
            ))
        })?;

        for (kind, amount_sat) in entries {
            let demand_asset = demand_asset_from_deferred(kind, issuance, material, &input.id)?;
            add_balance(&mut self.demand_by_asset, demand_asset, amount_sat)?;
        }

        Ok(())
    }

    /// Pick the currently largest positive deficit asset (tie-break by asset id ordering).
    fn pick_largest_deficit_asset(&self) -> Option<(AssetId, u64)> {
        self.current_deficits().iter().fold(
            None,
            |best: Option<(AssetId, u64)>, (asset, missing)| match best {
                None => Some((*asset, *missing)),
                Some((best_asset, best_missing)) => {
                    if *missing > best_missing || (*missing == best_missing && *asset < best_asset)
                    {
                        Some((*asset, *missing))
                    } else {
                        Some((best_asset, best_missing))
                    }
                }
            },
        )
    }

    async fn add_auxiliary_wallet_input(
        &mut self,
        pst: &mut PartiallySignedTransaction,
        selected: &WalletTxOut,
    ) -> Result<(), WalletAbiError> {
        if !self.used_outpoints.insert(selected.outpoint) {
            return Err(WalletAbiError::InvalidRequest(format!(
                "duplicate auxiliary outpoint resolved: {}:{}",
                selected.outpoint.txid, selected.outpoint.vout
            )));
        }

        let tx_out = self.wallet_meta.get_tx_out(selected.outpoint).await?;
        let mut pset_input = Input::from_prevout(selected.outpoint);
        pset_input.witness_utxo = Some(tx_out);
        pset_input.amount = Some(selected.unblinded.value);
        pset_input.asset = Some(selected.unblinded.asset);
        pset_input.proprietary.insert(
            get_finalizer_spec_key(),
            FinalizerSpec::Wallet.try_encode()?,
        );
        pset_input.proprietary.insert(
            get_secrets_spec_key(),
            serde_json::to_vec(&selected.unblinded)?,
        );
        let (pubkey, derivation_path) =
            self.signer_origin_for_wallet_utxo(WalletDerivationIndex {
                ext_int: selected.ext_int,
                wildcard_index: selected.wildcard_index,
            })?;
        pset_input
            .bip32_derivation
            .insert(pubkey, (self.signer_meta.fingerprint(), derivation_path));
        pst.add_input(pset_input);

        add_balance(
            &mut self.supply_by_asset,
            selected.unblinded.asset,
            selected.unblinded.value,
        )?;

        Ok(())
    }

    /// Select deterministic auxiliary wallet inputs for one deficit asset.
    ///
    /// Strategy order:
    /// 1. exact `BnB`
    /// 2. single largest-above-target
    /// 3. largest-first accumulation
    fn select_auxiliary_inputs_for_asset(
        &mut self,
        target_asset: AssetId,
        target_missing: u64,
    ) -> Result<Vec<WalletTxOut>, WalletAbiError> {
        let mut wallet_candidates: Vec<WalletTxOut> = self
            .wallet_snapshot
            .iter()
            .filter(|candidate| {
                !self.used_outpoints.contains(&candidate.outpoint)
                    && candidate.unblinded.asset == target_asset
            })
            .cloned()
            .collect();
        wallet_candidates.sort_by(|a, b| {
            b.unblinded
                .value
                .cmp(&a.unblinded.value)
                .then_with(|| {
                    a.outpoint
                        .txid
                        .to_string()
                        .cmp(&b.outpoint.txid.to_string())
                })
                .then_with(|| a.outpoint.vout.cmp(&b.outpoint.vout))
        });
        let available_candidates = wallet_candidates.len();
        let available_total_sat = wallet_candidates.iter().try_fold(0u64, |sum, candidate| {
            sum.checked_add(candidate.unblinded.value).ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "asset amount overflow while aggregating candidate pool for {}",
                    target_asset
                ))
            })
        })?;

        if wallet_candidates.is_empty() {
            return Err(funding_error_for_asset(
                target_asset,
                target_missing,
                available_candidates,
                available_total_sat,
            ));
        }

        let bnb_candidates = wallet_candidates
            .iter()
            .map(|candidate| BnbCandidate {
                amount_sat: candidate.unblinded.value,
                txid_lex: candidate.outpoint.txid.to_string(),
                vout: candidate.outpoint.vout,
            })
            .collect::<Vec<_>>();
        let bnb_selected_indices = bnb_exact_subset_indices(&bnb_candidates, target_missing)?;

        let selected_indices = if let Some(exact) = bnb_selected_indices {
            exact
        } else {
            let single_largest_selected =
                select_single_largest_above_target(&bnb_candidates, target_missing);

            if let Some(single) = single_largest_selected {
                single
            } else {
                let selected_indices =
                    select_largest_first_accumulation(&bnb_candidates, target_missing)?;

                if let Some(accumulated) = selected_indices {
                    accumulated
                } else {
                    return Err(funding_error_for_asset(
                        target_asset,
                        target_missing,
                        available_candidates,
                        available_total_sat,
                    ));
                }
            }
        };

        let selected_total = sum_selected_amount(&bnb_candidates, &selected_indices)?;

        if selected_total < target_missing {
            return Err(funding_error_for_asset(
                target_asset,
                target_missing,
                available_candidates,
                available_total_sat,
            ));
        }

        let selected = selected_indices
            .iter()
            .map(|index| wallet_candidates[*index].clone())
            .collect::<Vec<_>>();

        Ok(selected)
    }

    /// Add one or more auxiliary wallet inputs targeting one missing asset amount.
    ///
    /// The selected inputs are appended in deterministic order and each contribution updates
    /// `supply_by_asset` immediately.
    async fn add_auxiliary_input_for_asset(
        &mut self,
        pst: &mut PartiallySignedTransaction,
        target_asset: AssetId,
        target_missing: u64,
    ) -> Result<(), WalletAbiError> {
        let selected_inputs =
            self.select_auxiliary_inputs_for_asset(target_asset, target_missing)?;

        for selected in &selected_inputs {
            self.add_auxiliary_wallet_input(pst, selected).await?;
        }

        Ok(())
    }

    /// Repeatedly add auxiliary wallet inputs until there is no remaining positive deficit.
    ///
    /// Assets are processed by current largest deficit (asset-id tie-break).
    async fn add_auxiliary_inputs_until_balanced(
        &mut self,
        pst: &mut PartiallySignedTransaction,
    ) -> Result<(), WalletAbiError> {
        // Keep injecting auxiliary inputs until the equation has no remaining positive deficits.
        while let Some((target_asset, target_missing)) = self.pick_largest_deficit_asset() {
            self.add_auxiliary_input_for_asset(pst, target_asset, target_missing)
                .await?;
        }

        Ok(())
    }

    /// Return the best wallet UTXO candidate under a deterministic, deficit-aware score.
    ///
    /// Candidates must pass `WalletSourceFilter`, then are ranked lexicographically by:
    /// 1. total remaining deficit after simulated addition
    /// 2. remaining deficit on candidate asset
    /// 3. candidate overshoot/undershoot for that asset
    /// 4. `txid`, then `vout`
    ///
    /// # Complexity
    ///
    /// With `U` wallet UTXOs and `A` demanded assets, selection is `O(U * A)` time and `O(A)`
    /// temporary space per scored candidate simulation.
    fn filter_tx_out(
        &self,
        filter: &WalletSourceFilter,
    ) -> Result<Option<WalletTxOut>, WalletAbiError> {
        // Candidate ranking is lexicographic and fully deterministic:
        // 1) total remaining deficit after adding candidate
        // 2) remaining deficit on candidate's asset
        // 3) candidate overshoot/undershoot for that asset
        // 4) txid + vout tie-break
        let mut best: Option<(WalletTxOut, CandidateScore)> = None;

        for candidate in self
            .wallet_snapshot
            .iter()
            .filter(|x| self.matches_wallet_filter(x, filter))
        {
            let score = self.score_candidate(candidate)?;

            match &best {
                Some((_, best_score)) if score >= *best_score => {}
                _ => {
                    best = Some((candidate.clone(), score));
                }
            }
        }

        Ok(best.map(|(candidate, _)| candidate))
    }

    /// Score one candidate by simulating its supply contribution.
    ///
    /// Lower score tuple is better.
    fn score_candidate(&self, candidate: &WalletTxOut) -> Result<CandidateScore, WalletAbiError> {
        // Simulate adding this candidate to the current supply map, then compute a
        // deterministic lexicographic score that favors candidates which reduce deficits fastest.
        let mut simulated_supply = self.supply_by_asset.clone();
        let current_supply = simulated_supply
            .get(&candidate.unblinded.asset)
            .copied()
            .unwrap_or(0);
        let updated_supply = current_supply
            .checked_add(candidate.unblinded.value)
            .ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "asset amount overflow while scoring candidate {}:{}",
                    candidate.outpoint.txid, candidate.outpoint.vout
                ))
            })?;
        simulated_supply.insert(candidate.unblinded.asset, updated_supply);

        let mut total_remaining_deficit = 0u64;
        for (asset_id, demand_sat) in &self.demand_by_asset {
            let supplied = simulated_supply.get(asset_id).copied().unwrap_or(0);
            let remaining = demand_sat.saturating_sub(supplied);
            total_remaining_deficit =
                total_remaining_deficit
                    .checked_add(remaining)
                    .ok_or_else(|| {
                        WalletAbiError::InvalidRequest(
                            "deficit overflow while scoring wallet candidates".to_string(),
                        )
                    })?;
        }

        let candidate_demand = self
            .demand_by_asset
            .get(&candidate.unblinded.asset)
            .copied()
            .unwrap_or(0);
        let candidate_before_supply = self
            .supply_by_asset
            .get(&candidate.unblinded.asset)
            .copied()
            .unwrap_or(0);
        let candidate_after_supply = simulated_supply
            .get(&candidate.unblinded.asset)
            .copied()
            .unwrap_or(0);

        let remaining_candidate_deficit = candidate_demand.saturating_sub(candidate_after_supply);
        let needed_before = candidate_demand.saturating_sub(candidate_before_supply);
        let overshoot_or_undershoot = candidate.unblinded.value.abs_diff(needed_before);

        Ok((
            total_remaining_deficit,
            remaining_candidate_deficit,
            overshoot_or_undershoot,
            candidate.outpoint.txid.to_string(),
            candidate.outpoint.vout,
        ))
    }

    /// Check whether a wallet UTXO candidate satisfies source filters and is unused.
    fn matches_wallet_filter(&self, candidate: &WalletTxOut, filter: &WalletSourceFilter) -> bool {
        if candidate.is_spent || self.used_outpoints.contains(&candidate.outpoint) {
            return false;
        }

        let asset_ok = match filter.asset {
            AssetFilter::None => true,
            AssetFilter::Exact { asset_id } => candidate.unblinded.asset == asset_id,
        };
        if !asset_ok {
            return false;
        }

        let amount_ok = match filter.amount {
            AmountFilter::None => true,
            AmountFilter::Exact { amount_sat } => candidate.unblinded.value == amount_sat,
            AmountFilter::Min { amount_sat } => candidate.unblinded.value >= amount_sat,
        };
        if !amount_ok {
            return false;
        }

        match &filter.lock {
            LockFilter::None => true,
            LockFilter::Script { script } => candidate.script_pubkey == *script,
        }
    }

    /// Reserve an outpoint and fail if it was already used.
    fn reserve_outpoint(
        &mut self,
        input_id: &str,
        outpoint: OutPoint,
    ) -> Result<(), WalletAbiError> {
        if self.used_outpoints.insert(outpoint) {
            return Ok(());
        }

        Err(WalletAbiError::InvalidRequest(format!(
            "duplicate input outpoint resolved for '{}': {}:{}",
            input_id, outpoint.txid, outpoint.vout
        )))
    }

    /// Compute positive deficits `(demand - supply)` per asset.
    fn current_deficits(&self) -> BTreeMap<AssetId, u64> {
        let mut deficits = BTreeMap::new();

        for (&asset_id, &demand_sat) in &self.demand_by_asset {
            let supplied = self.supply_by_asset.get(&asset_id).copied().unwrap_or(0);

            if demand_sat > supplied {
                let _ = deficits.insert(asset_id, demand_sat - supplied);
            }
        }

        deficits
    }

    fn signer_origin_for_wallet_utxo(
        &self,
        index: WalletDerivationIndex,
    ) -> Result<(PublicKey, DerivationPath), WalletAbiError> {
        let ext_int = match index.ext_int {
            Chain::External => ChildNumber::from_normal_idx(0)?,
            Chain::Internal => ChildNumber::from_normal_idx(1)?,
        };
        let wildcard = ChildNumber::from_normal_idx(index.wildcard_index)?;

        let derivation_path = self
            .signer_meta
            .get_derivation_path(Bip84)
            .child(ext_int)
            .child(wildcard);
        let pubkey = self
            .signer_meta
            .get_pubkey_by_derivation_path(&derivation_path)?;

        Ok((pubkey, derivation_path))
    }
}

/// Validate that an output reference points to an existing declared input index.
fn validate_output_input_index(
    output_id: &str,
    input_index: u32,
    input_count: usize,
) -> Result<(), WalletAbiError> {
    let idx = usize::try_from(input_index)?;

    if idx >= input_count {
        return Err(WalletAbiError::InvalidRequest(format!(
            "output '{output_id}' references missing input_index {input_index}"
        )));
    }

    Ok(())
}

fn funding_error_for_asset(
    asset_id: AssetId,
    missing_sat: u64,
    candidate_count: usize,
    available_sat: u64,
) -> WalletAbiError {
    WalletAbiError::Funding(format!(
        "insufficient wallet funds for asset {asset_id}: missing {missing_sat} sat, available {available_sat} sat across {candidate_count} candidate UTXOs"
    ))
}

/// Add `amount_sat` to one asset bucket with overflow protection.
pub(crate) fn add_balance(
    map: &mut BTreeMap<AssetId, u64>,
    asset_id: AssetId,
    amount_sat: u64,
) -> Result<(), WalletAbiError> {
    match map.entry(asset_id) {
        Entry::Vacant(entry) => {
            entry.insert(amount_sat);
        }
        Entry::Occupied(mut entry) => {
            let v = entry.get_mut();
            *v = v.checked_add(amount_sat).ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "asset amount overflow while aggregating balances for {asset_id}"
                ))
            })?;
        }
    }

    Ok(())
}

/// Compute issuance entropy from input outpoint and issuance kind.
pub(crate) fn calculate_issuance_entropy(outpoint: OutPoint, issuance: &InputIssuance) -> Midstate {
    match issuance.kind {
        InputIssuanceKind::New => AssetId::generate_asset_entropy(
            outpoint,
            ContractHash::from_byte_array(issuance.entropy),
        ),
        InputIssuanceKind::Reissue => Midstate::from_byte_array(issuance.entropy),
    }
}

/// Resolve issuance token id for the current runtime issuance model.
///
/// This mirrors `elements::pset::Input::issuance_ids()` token derivation semantics where
/// the token confidentiality flag tracks `issuance_value_comm.is_some()`.
///
/// Runtime currently sets unblinded issuance amounts (`issuance_value_amount`) and does not
/// populate `issuance_value_comm`, so the confidentiality flag is intentionally fixed to `false`.
pub(crate) fn issuance_token_from_entropy_for_unblinded_issuance(
    issuance_entropy: Midstate,
) -> AssetId {
    let issuance_value_commitment_present = false;
    AssetId::reissuance_token_from_entropy(issuance_entropy, issuance_value_commitment_present)
}

/// Resolve a deferred issuance-linked output demand into a concrete asset id.
fn demand_asset_from_deferred(
    kind: DeferredDemandKind,
    issuance: &InputIssuance,
    material: &ResolvedInputMaterial,
    input_id: &str,
) -> Result<AssetId, WalletAbiError> {
    match (kind, &issuance.kind) {
        (DeferredDemandKind::NewIssuanceAsset, InputIssuanceKind::New)
        | (DeferredDemandKind::ReIssuanceAsset, InputIssuanceKind::Reissue) => Ok(
            AssetId::from_entropy(calculate_issuance_entropy(material.outpoint, issuance)),
        ),
        (DeferredDemandKind::NewIssuanceToken, InputIssuanceKind::New) => {
            Ok(issuance_token_from_entropy_for_unblinded_issuance(
                calculate_issuance_entropy(material.outpoint, issuance),
            ))
        }
        (DeferredDemandKind::NewIssuanceAsset, InputIssuanceKind::Reissue) => {
            Err(WalletAbiError::InvalidRequest(format!(
                "output asset variant new_issuance_asset references reissue input '{input_id}'"
            )))
        }
        (DeferredDemandKind::NewIssuanceToken, InputIssuanceKind::Reissue) => {
            Err(WalletAbiError::InvalidRequest(format!(
                "output asset variant new_issuance_token references reissue input '{input_id}'"
            )))
        }
        (DeferredDemandKind::ReIssuanceAsset, InputIssuanceKind::New) => {
            Err(WalletAbiError::InvalidRequest(format!(
                "output asset variant re_issuance_asset references new issuance input '{input_id}'"
            )))
        }
    }
}

/// Populate issuance-related PSET input fields from request metadata.
fn apply_issuance_to_pset_input(
    pset_input: &mut Input,
    issuance: &InputIssuance,
    material: &ResolvedInputMaterial,
) -> Result<(), WalletAbiError> {
    pset_input.issuance_value_amount = if issuance.asset_amount_sat == 0 {
        None
    } else {
        Some(issuance.asset_amount_sat)
    };
    // This entry is managed by the user
    pset_input.issuance_asset_entropy = Some(issuance.entropy);
    pset_input.issuance_inflation_keys = if issuance.token_amount_sat == 0 {
        None
    } else {
        Some(issuance.token_amount_sat)
    };

    if issuance.kind == InputIssuanceKind::Reissue {
        // Runtime currently emits unblinded issuance amounts; for reissuance we still need a
        // non-zero nonce and derive it from the input asset blinding factor.
        let mut nonce = material.secrets.asset_bf.into_inner();
        if nonce == secp256k1_zkp::ZERO_TWEAK {
            nonce = secp256k1_zkp::Tweak::from_slice(&ONE).map_err(|error| {
                WalletAbiError::InvalidRequest(format!(
                    "failed to construct non-zero reissuance blinding nonce: {error}"
                ))
            })?;
        }
        pset_input.issuance_blinding_nonce = Some(nonce);
    }

    pset_input.blinded_issuance = Some(0x00);

    Ok(())
}
