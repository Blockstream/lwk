use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{AssetVariant, InputSchema, RuntimeParams};
use crate::wallet_abi::tx_resolution::input_material::ResolvedInputMaterial;
use crate::wallet_abi::tx_resolution::utils::{
    add_balance, calculate_issuance_entropy, issuance_reference_asset_id,
    issuance_token_from_entropy_for_unblinded_issuance, validate_output_input_index,
};

use std::collections::{BTreeMap, BTreeSet, HashMap};

use log::error;

use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{AssetId, OutPoint};
use lwk_wollet::ExternalUtxo;

/// Aggregated amounts keyed by asset id.
pub(crate) type AssetBalances = BTreeMap<AssetId, u64>;

#[derive(Clone, Copy)]
enum DeferredDemandKind {
    NewIssuanceAsset,
    NewIssuanceToken,
    ReissueAsset,
}

#[derive(Clone, Copy)]
pub(super) enum IssuanceReferenceKind {
    NewAsset,
    NewToken,
    ReissueAsset,
}

pub(crate) struct SupplyAndDemand {
    demand_by_asset: BTreeMap<AssetId, u64>,
    supply_by_asset: BTreeMap<AssetId, u64>,
    deferred_demands: HashMap<u32, Vec<(DeferredDemandKind, u64)>>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CandidateScore {
    total_remaining_deficit: u64,
    remaining_candidate_deficit: u64,
    overshoot_or_undershoot: u64,
    txid_lex: String,
    vout: u32,
}

impl SupplyAndDemand {
    /// Build demand from output specs and store issuance-linked entries as deferred.
    ///
    /// Rules:
    /// - Non-fee outputs contribute demand directly (or deferred for issuance-derived assets).
    /// - Exactly one implicit policy-asset demand entry is added for `fee_target_sat`.
    pub(crate) fn try_from_runtime_params(
        params: &RuntimeParams,
        policy_asset: AssetId,
        fee_target_sat: u64,
    ) -> Result<Self, WalletAbiError> {
        let mut demand_by_asset: BTreeMap<AssetId, u64> = BTreeMap::new();
        let mut deferred_demands: HashMap<u32, Vec<(DeferredDemandKind, u64)>> = HashMap::new();

        for output in &params.outputs {
            match &output.asset {
                AssetVariant::AssetId { asset_id } => {
                    add_balance(&mut demand_by_asset, *asset_id, output.amount_sat)?;
                }
                AssetVariant::NewIssuanceAsset { input_index } => {
                    validate_output_input_index(&output.id, *input_index, params.inputs.len())?;
                    deferred_demands
                        .entry(*input_index)
                        .or_default()
                        .push((DeferredDemandKind::NewIssuanceAsset, output.amount_sat));
                }
                AssetVariant::NewIssuanceToken { input_index } => {
                    validate_output_input_index(&output.id, *input_index, params.inputs.len())?;
                    deferred_demands
                        .entry(*input_index)
                        .or_default()
                        .push((DeferredDemandKind::NewIssuanceToken, output.amount_sat));
                }
                AssetVariant::ReIssuanceAsset { input_index } => {
                    validate_output_input_index(&output.id, *input_index, params.inputs.len())?;
                    deferred_demands
                        .entry(*input_index)
                        .or_default()
                        .push((DeferredDemandKind::ReissueAsset, output.amount_sat));
                }
            }
        }

        // Fee demand is always modeled from runtime target, independent of params fee amount.
        add_balance(&mut demand_by_asset, policy_asset, fee_target_sat)?;

        Ok(Self {
            demand_by_asset,
            deferred_demands,
            supply_by_asset: Default::default(),
        })
    }

    pub(crate) fn apply_resolved_input_contribution(
        &mut self,
        input: &InputSchema,
        input_index: usize,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        self.apply_input_supply(input, material)?;
        self.activate_deferred_demands_for_input(input, input_index, material)
    }

    /// Pick the currently largest positive deficit asset (tie-break by asset id ordering).
    pub(crate) fn pick_largest_deficit_asset(&self) -> Option<(AssetId, u64)> {
        self.demand_by_asset.iter().fold(
            None,
            |best: Option<(AssetId, u64)>, (asset, demand_sat)| {
                let supplied = self.supply_by_asset.get(asset).copied().unwrap_or(0);
                let missing = demand_sat.saturating_sub(supplied);
                if missing == 0 {
                    return best;
                }

                match best {
                    None => Some((*asset, missing)),
                    Some((best_asset, best_missing)) => {
                        if missing > best_missing
                            || (missing == best_missing && *asset < best_asset)
                        {
                            Some((*asset, missing))
                        } else {
                            Some((best_asset, best_missing))
                        }
                    }
                }
            },
        )
    }

    pub(crate) fn add_selected_wallet_to_supply(
        &mut self,
        selected_wallet_utxo: &ExternalUtxo,
    ) -> Result<(), WalletAbiError> {
        add_balance(
            &mut self.supply_by_asset,
            selected_wallet_utxo.unblinded.asset,
            selected_wallet_utxo.unblinded.value,
        )
    }

    /// Apply the resolved input contribution to equation supply (base + issuance minting).
    fn apply_input_supply(
        &mut self,
        input: &InputSchema,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        add_balance(
            &mut self.supply_by_asset,
            material.secrets().asset,
            material.secrets().value,
        )?;

        if let Some(issuance) = input.issuance.as_ref() {
            let issuance_entropy = calculate_issuance_entropy(*material.outpoint(), issuance);
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

    /// Convert deferred issuance-linked demand into concrete asset demand for one input index.
    fn activate_deferred_demands_for_input(
        &mut self,
        input: &InputSchema,
        input_index: usize,
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
            let reference_kind = match kind {
                DeferredDemandKind::NewIssuanceAsset => IssuanceReferenceKind::NewAsset,
                DeferredDemandKind::NewIssuanceToken => IssuanceReferenceKind::NewToken,
                DeferredDemandKind::ReissueAsset => IssuanceReferenceKind::ReissueAsset,
            };
            let demand_asset = issuance_reference_asset_id(
                reference_kind,
                issuance,
                *material.outpoint(),
                || match reference_kind {
                    IssuanceReferenceKind::NewAsset => WalletAbiError::InvalidRequest(format!(
                        "output asset variant new_issuance_asset references reissue input '{}'",
                        input.id
                    )),
                    IssuanceReferenceKind::NewToken => WalletAbiError::InvalidRequest(format!(
                        "output asset variant new_issuance_token references reissue input '{}'",
                        input.id
                    )),
                    IssuanceReferenceKind::ReissueAsset => WalletAbiError::InvalidRequest(format!(
                        "output asset variant re_issuance_asset references new issuance input '{}'",
                        input.id
                    )),
                },
            )?;
            add_balance(&mut self.demand_by_asset, demand_asset, amount_sat)?;
        }

        Ok(())
    }

    pub(crate) fn validate_demand_after_resolution(&self) -> Result<(), WalletAbiError> {
        if !self.deferred_demands.is_empty() {
            return Err(WalletAbiError::InvalidRequest(
                "unresolved deferred output demands remain after input resolution".to_string(),
            ));
        }

        Ok(())
    }

    pub(crate) fn score_candidate(
        &self,
        candidate: &ExternalUtxo,
        current_total_deficit: u64,
    ) -> Result<CandidateScore, WalletAbiError> {
        let candidate_asset = candidate.unblinded.asset;
        let candidate_demand = self
            .demand_by_asset
            .get(&candidate_asset)
            .copied()
            .unwrap_or(0);
        let candidate_before_supply = self
            .supply_by_asset
            .get(&candidate_asset)
            .copied()
            .unwrap_or(0);
        let candidate_after_supply = candidate_before_supply
            .checked_add(candidate.unblinded.value)
            .ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "asset amount overflow while scoring candidate {}:{}",
                    candidate.outpoint.txid, candidate.outpoint.vout
                ))
            })?;

        let remaining_candidate_deficit = candidate_demand.saturating_sub(candidate_after_supply);
        let needed_before = candidate_demand.saturating_sub(candidate_before_supply);
        let total_remaining_deficit = current_total_deficit
            .checked_sub(needed_before - remaining_candidate_deficit)
            .ok_or_else(|| {
                WalletAbiError::InvalidRequest(
                    "deficit underflow while scoring wallet candidates".to_string(),
                )
            })?;

        Ok(CandidateScore {
            total_remaining_deficit,
            remaining_candidate_deficit,
            overshoot_or_undershoot: candidate.unblinded.value.abs_diff(needed_before),
            txid_lex: candidate.outpoint.txid.to_string(),
            vout: candidate.outpoint.vout,
        })
    }

    pub(crate) fn total_remaining_deficit(&self) -> Result<u64, WalletAbiError> {
        self.demand_by_asset
            .iter()
            .try_fold(0u64, |sum, (asset_id, demand_sat)| {
                let supplied = self.supply_by_asset.get(asset_id).copied().unwrap_or(0);
                sum.checked_add(demand_sat.saturating_sub(supplied))
                    .ok_or_else(|| {
                        WalletAbiError::InvalidRequest(
                            "deficit overflow while scoring wallet candidates".to_string(),
                        )
                    })
            })
    }

    /// Aggregate total per-asset input supply.
    ///
    /// Supply is the sum of:
    /// - Base amounts from all PSET inputs.
    /// - Minted issuance/reissuance amounts derived from declared input metadata.
    ///
    /// Overflow is rejected via checked arithmetic.
    pub(crate) fn aggregate_input_supply(
        pst: &PartiallySignedTransaction,
        params: &RuntimeParams,
    ) -> Result<AssetBalances, WalletAbiError> {
        let mut balances = AssetBalances::new();

        for (input_index, input) in pst.inputs().iter().enumerate() {
            let asset = input.asset.ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "input index {input_index} missing explicit asset while aggregating supply"
                ))
            })?;
            let amount_sat = input.amount.ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "input index {input_index} missing explicit amount while aggregating supply"
                ))
            })?;
            add_balance(&mut balances, asset, amount_sat)?;
        }

        let issuance_supply = Self::aggregate_issuance_supply(pst, params)?;
        for (asset_id, amount_sat) in issuance_supply {
            add_balance(&mut balances, asset_id, amount_sat)?;
        }

        Ok(balances)
    }

    /// Aggregate total per-asset output demand from current PSET outputs.
    ///
    /// Fee output (policy asset, empty script) is treated as ordinary demand and is not
    /// special-cased in this aggregation.
    pub(crate) fn aggregate_output_demand(
        pst: &PartiallySignedTransaction,
    ) -> Result<AssetBalances, WalletAbiError> {
        let mut balances = AssetBalances::new();

        for (output_index, output) in pst.outputs().iter().enumerate() {
            let asset = output.asset.ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "output index {output_index} missing explicit asset while aggregating demand"
                ))
            })?;
            let amount_sat = output.amount.ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "output index {output_index} missing explicit amount while aggregating demand"
                ))
            })?;
            add_balance(&mut balances, asset, amount_sat)?;
        }

        Ok(balances)
    }

    pub(crate) fn residuals_or_funding_error(
        supply_by_asset: &AssetBalances,
        demand_by_asset: &AssetBalances,
        fee_target_sat: u64,
    ) -> Result<AssetBalances, WalletAbiError> {
        let mut residual_by_asset = AssetBalances::new();
        let mut deficit_by_asset = AssetBalances::new();
        let mut all_assets = BTreeSet::new();

        all_assets.extend(supply_by_asset.keys().copied());
        all_assets.extend(demand_by_asset.keys().copied());

        for asset_id in all_assets {
            let supply_sat = supply_by_asset.get(&asset_id).copied().unwrap_or(0);
            let demand_sat = demand_by_asset.get(&asset_id).copied().unwrap_or(0);

            if demand_sat > supply_sat {
                deficit_by_asset.insert(asset_id, demand_sat - supply_sat);
                continue;
            }

            if supply_sat > demand_sat {
                residual_by_asset.insert(asset_id, supply_sat - demand_sat);
            }
        }

        if !deficit_by_asset.is_empty() {
            let details = deficit_by_asset
                .iter()
                .map(|(asset_id, missing_sat)| format!("{asset_id}:{missing_sat}"))
                .collect::<Vec<_>>()
                .join(", ");

            error!("asset deficits after applying fee target {fee_target_sat}: {details}");

            return Err(WalletAbiError::Funding(
                "asset deficits after applying fee target".to_string(),
            ));
        }

        Ok(residual_by_asset)
    }

    /// Final safety check asserting exact per-asset conservation after change materialization.
    ///
    /// This enforces `supply[a] == demand[a]` for every asset `a`.
    pub(crate) fn assert_exact_asset_conservation(
        pst: &PartiallySignedTransaction,
        params: &RuntimeParams,
    ) -> Result<(), WalletAbiError> {
        let supply_by_asset = Self::aggregate_input_supply(pst, params)?;
        let demand_by_asset = Self::aggregate_output_demand(pst)?;
        let mut all_assets = BTreeSet::new();
        let mut mismatches = Vec::new();

        all_assets.extend(supply_by_asset.keys().copied());
        all_assets.extend(demand_by_asset.keys().copied());

        for asset_id in all_assets {
            let supply_sat = supply_by_asset.get(&asset_id).copied().unwrap_or(0);
            let demand_sat = demand_by_asset.get(&asset_id).copied().unwrap_or(0);
            if supply_sat != demand_sat {
                mismatches.push(format!(
                    "{asset_id}:supply={supply_sat},demand={demand_sat}"
                ));
            }
        }

        if mismatches.is_empty() {
            return Ok(());
        }

        error!(
            "Asset conservation violated after balancing: {:#?}",
            mismatches
        );

        Err(WalletAbiError::InvalidRequest(
            "asset conservation violated after balancing".to_string(),
        ))
    }

    /// Aggregate issuance/reissuance minting supply from declared inputs.
    ///
    /// For each declared input with issuance metadata:
    /// - Add `asset_amount_sat` to the derived issuance asset id.
    /// - Add `token_amount_sat` to the derived reissuance token id (if non-zero).
    fn aggregate_issuance_supply(
        pst: &PartiallySignedTransaction,
        params: &RuntimeParams,
    ) -> Result<AssetBalances, WalletAbiError> {
        let mut balances = AssetBalances::new();

        for (input_index, input) in params.inputs.iter().enumerate() {
            let Some(issuance) = input.issuance.as_ref() else {
                continue;
            };

            let pset_input = pst.inputs().get(input_index).ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "input '{}' at index {input_index} missing from PSET while aggregating issuance supply",
                    input.id
                ))
            })?;

            let outpoint =
                OutPoint::new(pset_input.previous_txid, pset_input.previous_output_index);
            let entropy = calculate_issuance_entropy(outpoint, issuance);
            let issuance_asset = AssetId::from_entropy(entropy);
            add_balance(&mut balances, issuance_asset, issuance.asset_amount_sat)?;

            if issuance.token_amount_sat > 0 {
                let token_asset = issuance_token_from_entropy_for_unblinded_issuance(entropy);
                add_balance(&mut balances, token_asset, issuance.token_amount_sat)?;
            }
        }

        Ok(balances)
    }
}
