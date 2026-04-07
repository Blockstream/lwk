use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{AssetVariant, InputSchema, RuntimeParams};
use crate::wallet_abi::tx_resolution::resolved_input::ResolvedInputMaterial;
use crate::wallet_abi::tx_resolution::utils::{
    add_balance, calculate_issuance_entropy, issuance_reference_asset_id,
    issuance_token_from_entropy_for_unblinded_issuance, validate_output_input_index,
    IssuanceReferenceKind,
};

use std::collections::{BTreeMap, BTreeSet, HashMap};

use log::error;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{AssetId, OutPoint};
use lwk_wollet::ExternalUtxo;

pub(crate) type AssetBalances = BTreeMap<AssetId, u64>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeferredDemandKind {
    NewIssuanceAsset,
    NewIssuanceToken,
    ReissueAsset,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CandidateScore {
    total_remaining_deficit: u64,
    remaining_candidate_deficit: u64,
    overshoot_or_undershoot: u64,
    txid_lex: String,
    vout: u32,
}

pub(crate) struct SupplyAndDemand {
    demand_by_asset: AssetBalances,
    supply_by_asset: AssetBalances,
    deferred_demands: HashMap<u32, Vec<(DeferredDemandKind, u64)>>,
}

impl SupplyAndDemand {
    pub(crate) fn try_from_runtime_params(
        params: &RuntimeParams,
        policy_asset: AssetId,
        fee_target_sat: u64,
    ) -> Result<Self, WalletAbiError> {
        let mut demand_by_asset = AssetBalances::new();
        let mut deferred_demands = HashMap::new();

        for output in &params.outputs {
            match output.asset {
                AssetVariant::AssetId { asset_id } => {
                    add_balance(&mut demand_by_asset, asset_id, output.amount_sat)?;
                }
                AssetVariant::NewIssuanceAsset { input_index } => {
                    validate_output_input_index(&output.id, input_index, params.inputs.len())?;
                    deferred_demands
                        .entry(input_index)
                        .or_insert_with(Vec::new)
                        .push((DeferredDemandKind::NewIssuanceAsset, output.amount_sat));
                }
                AssetVariant::NewIssuanceToken { input_index } => {
                    validate_output_input_index(&output.id, input_index, params.inputs.len())?;
                    deferred_demands
                        .entry(input_index)
                        .or_insert_with(Vec::new)
                        .push((DeferredDemandKind::NewIssuanceToken, output.amount_sat));
                }
                AssetVariant::ReIssuanceAsset { input_index } => {
                    validate_output_input_index(&output.id, input_index, params.inputs.len())?;
                    deferred_demands
                        .entry(input_index)
                        .or_insert_with(Vec::new)
                        .push((DeferredDemandKind::ReissueAsset, output.amount_sat));
                }
            }
        }

        add_balance(&mut demand_by_asset, policy_asset, fee_target_sat)?;

        Ok(Self {
            demand_by_asset,
            supply_by_asset: AssetBalances::new(),
            deferred_demands,
        })
    }

    pub(crate) fn pick_largest_deficit_asset(&self) -> Option<(AssetId, u64)> {
        self.demand_by_asset.iter().fold(
            None,
            |best: Option<(AssetId, u64)>, (asset_id, demand_sat)| {
                let supplied = self.supply_by_asset.get(asset_id).copied().unwrap_or(0);
                let missing = demand_sat.saturating_sub(supplied);
                if missing == 0 {
                    return best;
                }

                match best {
                    None => Some((*asset_id, missing)),
                    Some((best_asset_id, best_missing)) => {
                        if missing > best_missing
                            || (missing == best_missing && *asset_id < best_asset_id)
                        {
                            Some((*asset_id, missing))
                        } else {
                            Some((best_asset_id, best_missing))
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

    pub(crate) fn total_remaining_deficit(&self) -> Result<u64, WalletAbiError> {
        self.demand_by_asset
            .iter()
            .try_fold(0u64, |sum, (asset_id, demand_sat)| {
                let supplied = self.supply_by_asset.get(asset_id).copied().unwrap_or(0);
                sum.checked_add(demand_sat.saturating_sub(supplied))
                    .ok_or_else(|| {
                        WalletAbiError::InvalidRequest(
                            "deficit overflow while scoring wallet candidates".to_owned(),
                        )
                    })
            })
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
                    "deficit underflow while scoring wallet candidates".to_owned(),
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

    pub(crate) fn apply_input_supply(
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
            if matches!(
                issuance.kind,
                crate::wallet_abi::schema::InputIssuanceKind::Reissue
            ) && issuance.token_amount_sat > 0
            {
                return Err(WalletAbiError::InvalidRequest(
                    "reissuance cannot create new reissuance tokens".to_owned(),
                ));
            }

            let issuance_entropy = calculate_issuance_entropy(material.outpoint, issuance);
            add_balance(
                &mut self.supply_by_asset,
                AssetId::from_entropy(issuance_entropy),
                issuance.asset_amount_sat,
            )?;

            if issuance.token_amount_sat > 0 {
                add_balance(
                    &mut self.supply_by_asset,
                    issuance_token_from_entropy_for_unblinded_issuance(issuance_entropy),
                    issuance.token_amount_sat,
                )?;
            }
        }

        Ok(())
    }

    pub(crate) fn activate_deferred_demands_for_input(
        &mut self,
        input: &InputSchema,
        input_index: usize,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        let input_index = u32::try_from(input_index)?;
        let Some(entries) = self.deferred_demands.remove(&input_index) else {
            return Ok(());
        };

        let issuance = input.issuance.as_ref().ok_or_else(|| {
            WalletAbiError::InvalidRequest(format!(
                "output asset references input {input_index} but input '{}' has no issuance metadata",
                input.id
            ))
        })?;

        for (kind, amount_sat) in entries {
            let reference_kind = match kind {
                DeferredDemandKind::NewIssuanceAsset => IssuanceReferenceKind::NewAsset,
                DeferredDemandKind::NewIssuanceToken => IssuanceReferenceKind::NewToken,
                DeferredDemandKind::ReissueAsset => IssuanceReferenceKind::ReissueAsset,
            };
            let demand_asset =
                issuance_reference_asset_id(reference_kind, issuance, material.outpoint, || {
                    match reference_kind {
                        IssuanceReferenceKind::NewAsset => WalletAbiError::InvalidRequest(format!(
                            "output asset variant new_issuance_asset references reissue input '{}'",
                            input.id
                        )),
                        IssuanceReferenceKind::NewToken => WalletAbiError::InvalidRequest(format!(
                            "output asset variant new_issuance_token references reissue input '{}'",
                            input.id
                        )),
                        IssuanceReferenceKind::ReissueAsset => {
                            WalletAbiError::InvalidRequest(format!(
                        "output asset variant re_issuance_asset references new issuance input '{}'",
                        input.id
                    ))
                        }
                    }
                })?;
            add_balance(&mut self.demand_by_asset, demand_asset, amount_sat)?;
        }

        Ok(())
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

    pub(crate) fn validate_demand_after_resolution(&self) -> Result<(), WalletAbiError> {
        if !self.deferred_demands.is_empty() {
            return Err(WalletAbiError::InvalidRequest(
                "unresolved deferred output demands remain after input resolution".to_owned(),
            ));
        }

        Ok(())
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
                "asset deficits after applying fee target".to_owned(),
            ));
        }

        Ok(residual_by_asset)
    }

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
            add_balance(
                &mut balances,
                AssetId::from_entropy(entropy),
                issuance.asset_amount_sat,
            )?;

            if issuance.token_amount_sat > 0 {
                add_balance(
                    &mut balances,
                    issuance_token_from_entropy_for_unblinded_issuance(entropy),
                    issuance.token_amount_sat,
                )?;
            }
        }

        Ok(balances)
    }

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
}

#[cfg(test)]
mod tests {
    use super::{AssetBalances, DeferredDemandKind, SupplyAndDemand};
    use crate::wallet_abi::schema::{
        AssetVariant, BlinderVariant, InputIssuance, InputIssuanceKind, InputSchema, LockVariant,
        OutputSchema, RuntimeParams,
    };
    use crate::wallet_abi::tx_resolution::resolved_input::ResolvedInputMaterial;
    use crate::wallet_abi::tx_resolution::utils::{
        calculate_issuance_entropy, issuance_token_from_entropy_for_unblinded_issuance,
    };

    use lwk_wollet::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use lwk_wollet::elements::pset::{Input, Output, PartiallySignedTransaction};
    use lwk_wollet::elements::{AssetId, OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid};
    use lwk_wollet::ExternalUtxo;

    #[test]
    fn runtime_params_add_policy_and_explicit_asset_demand() {
        let policy_asset = AssetId::LIQUID_BTC;
        let issued_asset = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let params = RuntimeParams {
            inputs: vec![],
            outputs: vec![
                OutputSchema {
                    id: "policy".to_owned(),
                    amount_sat: 21,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::AssetId {
                        asset_id: policy_asset,
                    },
                    blinder: BlinderVariant::Wallet,
                },
                OutputSchema {
                    id: "issued".to_owned(),
                    amount_sat: 8,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::AssetId {
                        asset_id: issued_asset,
                    },
                    blinder: BlinderVariant::Wallet,
                },
            ],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };

        let supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, policy_asset, 13).unwrap();

        assert_eq!(
            supply_and_demand.demand_by_asset.get(&policy_asset),
            Some(&34)
        );
        assert_eq!(
            supply_and_demand.demand_by_asset.get(&issued_asset),
            Some(&8)
        );
        assert!(supply_and_demand.supply_by_asset.is_empty());
        assert!(supply_and_demand.deferred_demands.is_empty());
    }

    #[test]
    fn issuance_linked_outputs_are_deferred() {
        let params = RuntimeParams {
            inputs: vec![InputSchema::new("issuer")],
            outputs: vec![
                OutputSchema {
                    id: "asset".to_owned(),
                    amount_sat: 5,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::NewIssuanceAsset { input_index: 0 },
                    blinder: BlinderVariant::Wallet,
                },
                OutputSchema {
                    id: "token".to_owned(),
                    amount_sat: 7,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::NewIssuanceToken { input_index: 0 },
                    blinder: BlinderVariant::Wallet,
                },
            ],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };

        let supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, AssetId::LIQUID_BTC, 3).unwrap();

        assert_eq!(
            supply_and_demand.demand_by_asset.get(&AssetId::LIQUID_BTC),
            Some(&3)
        );
        assert_eq!(
            supply_and_demand.deferred_demands.get(&0),
            Some(&vec![
                (DeferredDemandKind::NewIssuanceAsset, 5),
                (DeferredDemandKind::NewIssuanceToken, 7),
            ])
        );

        assert!(SupplyAndDemand::try_from_runtime_params(
            &RuntimeParams {
                inputs: vec![],
                outputs: vec![OutputSchema {
                    id: "bad".to_owned(),
                    amount_sat: 1,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::NewIssuanceAsset { input_index: 0 },
                    blinder: BlinderVariant::Wallet,
                }],
                fee_rate_sat_kvb: None,
                lock_time: None,
            },
            AssetId::LIQUID_BTC,
            0
        )
        .is_err());
    }

    #[test]
    fn largest_deficit_asset_selection_is_deterministic() {
        let lower_asset = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let higher_asset = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a471"
            .parse::<AssetId>()
            .unwrap();
        let params = RuntimeParams {
            inputs: vec![],
            outputs: vec![
                OutputSchema {
                    id: "higher-first".to_owned(),
                    amount_sat: 7,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::AssetId {
                        asset_id: higher_asset,
                    },
                    blinder: BlinderVariant::Wallet,
                },
                OutputSchema {
                    id: "lower-second".to_owned(),
                    amount_sat: 7,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::AssetId {
                        asset_id: lower_asset,
                    },
                    blinder: BlinderVariant::Wallet,
                },
            ],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };

        let supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, lower_asset, 0).unwrap();

        assert_eq!(
            supply_and_demand.pick_largest_deficit_asset(),
            Some((lower_asset, 7))
        );
    }

    #[test]
    fn wallet_supply_updates_reduce_remaining_deficit() {
        let asset_id = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let params = RuntimeParams {
            inputs: vec![],
            outputs: vec![OutputSchema {
                id: "need".to_owned(),
                amount_sat: 11,
                lock: LockVariant::Wallet,
                asset: AssetVariant::AssetId { asset_id },
                blinder: BlinderVariant::Wallet,
            }],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let mut supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, asset_id, 0).unwrap();
        let selected_wallet_utxo = ExternalUtxo {
            outpoint: OutPoint::new(
                "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                    .parse::<Txid>()
                    .unwrap(),
                0,
            ),
            txout: TxOut::default(),
            tx: None::<Transaction>,
            unblinded: TxOutSecrets::new(
                asset_id,
                AssetBlindingFactor::zero(),
                7,
                ValueBlindingFactor::zero(),
            ),
            max_weight_to_satisfy: 0,
        };

        assert_eq!(
            supply_and_demand.pick_largest_deficit_asset(),
            Some((asset_id, 11))
        );

        supply_and_demand
            .add_selected_wallet_to_supply(&selected_wallet_utxo)
            .unwrap();

        assert_eq!(
            supply_and_demand.pick_largest_deficit_asset(),
            Some((asset_id, 4))
        );
    }

    #[test]
    fn total_remaining_deficit_sums_shortfalls() {
        let asset_a = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let asset_b = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a471"
            .parse::<AssetId>()
            .unwrap();
        let params = RuntimeParams {
            inputs: vec![],
            outputs: vec![
                OutputSchema {
                    id: "need-a".to_owned(),
                    amount_sat: 10,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::AssetId { asset_id: asset_a },
                    blinder: BlinderVariant::Wallet,
                },
                OutputSchema {
                    id: "need-b".to_owned(),
                    amount_sat: 6,
                    lock: LockVariant::Wallet,
                    asset: AssetVariant::AssetId { asset_id: asset_b },
                    blinder: BlinderVariant::Wallet,
                },
            ],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let mut supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, asset_a, 0).unwrap();

        supply_and_demand
            .add_selected_wallet_to_supply(&ExternalUtxo {
                outpoint: OutPoint::new(
                    "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                        .parse::<Txid>()
                        .unwrap(),
                    1,
                ),
                txout: TxOut::default(),
                tx: None::<Transaction>,
                unblinded: TxOutSecrets::new(
                    asset_a,
                    AssetBlindingFactor::zero(),
                    4,
                    ValueBlindingFactor::zero(),
                ),
                max_weight_to_satisfy: 0,
            })
            .unwrap();

        assert_eq!(supply_and_demand.total_remaining_deficit().unwrap(), 12);
    }

    #[test]
    fn candidate_scoring_tie_breaks_deterministically() {
        let asset_id = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let params = RuntimeParams {
            inputs: vec![],
            outputs: vec![OutputSchema {
                id: "need".to_owned(),
                amount_sat: 100,
                lock: LockVariant::Wallet,
                asset: AssetVariant::AssetId { asset_id },
                blinder: BlinderVariant::Wallet,
            }],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, asset_id, 0).unwrap();
        let total_deficit = supply_and_demand.total_remaining_deficit().unwrap();
        let lower_txid = ExternalUtxo {
            outpoint: OutPoint::new(
                "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                    .parse::<Txid>()
                    .unwrap(),
                0,
            ),
            txout: TxOut::default(),
            tx: None::<Transaction>,
            unblinded: TxOutSecrets::new(
                asset_id,
                AssetBlindingFactor::zero(),
                60,
                ValueBlindingFactor::zero(),
            ),
            max_weight_to_satisfy: 0,
        };
        let higher_txid = ExternalUtxo {
            outpoint: OutPoint::new(
                "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a471"
                    .parse::<Txid>()
                    .unwrap(),
                0,
            ),
            txout: TxOut::default(),
            tx: None::<Transaction>,
            unblinded: TxOutSecrets::new(
                asset_id,
                AssetBlindingFactor::zero(),
                60,
                ValueBlindingFactor::zero(),
            ),
            max_weight_to_satisfy: 0,
        };

        assert!(
            supply_and_demand
                .score_candidate(&lower_txid, total_deficit)
                .unwrap()
                < supply_and_demand
                    .score_candidate(&higher_txid, total_deficit)
                    .unwrap()
        );
    }

    #[test]
    fn input_supply_adds_prevout_value() {
        let prevout_asset = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let mut supply_and_demand = SupplyAndDemand::try_from_runtime_params(
            &RuntimeParams {
                inputs: vec![],
                outputs: vec![],
                fee_rate_sat_kvb: None,
                lock_time: None,
            },
            AssetId::LIQUID_BTC,
            0,
        )
        .unwrap();

        supply_and_demand
            .apply_input_supply(
                &InputSchema::new("plain"),
                &ResolvedInputMaterial {
                    outpoint: OutPoint::new(
                        "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                            .parse::<Txid>()
                            .unwrap(),
                        0,
                    ),
                    secrets: TxOutSecrets::new(
                        prevout_asset,
                        AssetBlindingFactor::zero(),
                        9,
                        ValueBlindingFactor::zero(),
                    ),
                },
            )
            .unwrap();

        assert_eq!(
            supply_and_demand.supply_by_asset.get(&prevout_asset),
            Some(&9)
        );
    }

    #[test]
    fn input_supply_adds_issuance_amounts() {
        let issuance_input = InputSchema::new("issuer").with_issuance(InputIssuance {
            kind: InputIssuanceKind::New,
            asset_amount_sat: 3,
            token_amount_sat: 2,
            entropy: [7; 32],
        });
        let resolved = ResolvedInputMaterial {
            outpoint: OutPoint::new(
                "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                    .parse::<Txid>()
                    .unwrap(),
                1,
            ),
            secrets: TxOutSecrets::new(
                AssetId::LIQUID_BTC,
                AssetBlindingFactor::zero(),
                0,
                ValueBlindingFactor::zero(),
            ),
        };
        let mut supply_and_demand = SupplyAndDemand::try_from_runtime_params(
            &RuntimeParams {
                inputs: vec![],
                outputs: vec![],
                fee_rate_sat_kvb: None,
                lock_time: None,
            },
            AssetId::LIQUID_BTC,
            0,
        )
        .unwrap();

        supply_and_demand
            .apply_input_supply(&issuance_input, &resolved)
            .unwrap();

        let entropy = calculate_issuance_entropy(
            resolved.outpoint,
            issuance_input.issuance.as_ref().unwrap(),
        );
        assert_eq!(
            supply_and_demand
                .supply_by_asset
                .get(&AssetId::from_entropy(entropy)),
            Some(&3)
        );
        assert_eq!(
            supply_and_demand
                .supply_by_asset
                .get(&issuance_token_from_entropy_for_unblinded_issuance(entropy)),
            Some(&2)
        );
    }

    #[test]
    fn deferred_issuance_demand_activates_after_input_resolution() {
        let params = RuntimeParams {
            inputs: vec![InputSchema::new("issuer").with_issuance(InputIssuance {
                kind: InputIssuanceKind::New,
                asset_amount_sat: 0,
                token_amount_sat: 0,
                entropy: [7; 32],
            })],
            outputs: vec![OutputSchema {
                id: "issued-output".to_owned(),
                amount_sat: 5,
                lock: LockVariant::Wallet,
                asset: AssetVariant::NewIssuanceAsset { input_index: 0 },
                blinder: BlinderVariant::Wallet,
            }],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let resolved = ResolvedInputMaterial {
            outpoint: OutPoint::new(
                "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                    .parse::<Txid>()
                    .unwrap(),
                0,
            ),
            secrets: TxOutSecrets::new(
                AssetId::LIQUID_BTC,
                AssetBlindingFactor::zero(),
                9,
                ValueBlindingFactor::zero(),
            ),
        };
        let mut supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, AssetId::LIQUID_BTC, 0).unwrap();

        assert_eq!(supply_and_demand.pick_largest_deficit_asset(), None);

        supply_and_demand
            .activate_deferred_demands_for_input(&params.inputs[0], 0, &resolved)
            .unwrap();

        let entropy = calculate_issuance_entropy(
            resolved.outpoint,
            params.inputs[0].issuance.as_ref().unwrap(),
        );
        assert_eq!(
            supply_and_demand.pick_largest_deficit_asset(),
            Some((AssetId::from_entropy(entropy), 5))
        );
    }

    #[test]
    fn activation_rejects_missing_issuance_metadata() {
        let params = RuntimeParams {
            inputs: vec![InputSchema::new("plain-input")],
            outputs: vec![OutputSchema {
                id: "bad-ref".to_owned(),
                amount_sat: 1,
                lock: LockVariant::Wallet,
                asset: AssetVariant::NewIssuanceAsset { input_index: 0 },
                blinder: BlinderVariant::Wallet,
            }],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let mut supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, AssetId::LIQUID_BTC, 0).unwrap();

        assert!(supply_and_demand
            .activate_deferred_demands_for_input(
                &params.inputs[0],
                0,
                &ResolvedInputMaterial {
                    outpoint: OutPoint::new(
                        "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                            .parse::<Txid>()
                            .unwrap(),
                        1,
                    ),
                    secrets: TxOutSecrets::new(
                        AssetId::LIQUID_BTC,
                        AssetBlindingFactor::zero(),
                        1,
                        ValueBlindingFactor::zero(),
                    ),
                },
            )
            .is_err());
    }

    #[test]
    fn issuance_supply_satisfies_activated_deferred_demand() {
        let params = RuntimeParams {
            inputs: vec![InputSchema::new("issuer").with_issuance(InputIssuance {
                kind: InputIssuanceKind::New,
                asset_amount_sat: 5,
                token_amount_sat: 0,
                entropy: [7; 32],
            })],
            outputs: vec![OutputSchema {
                id: "issued-output".to_owned(),
                amount_sat: 5,
                lock: LockVariant::Wallet,
                asset: AssetVariant::NewIssuanceAsset { input_index: 0 },
                blinder: BlinderVariant::Wallet,
            }],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let mut supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, AssetId::LIQUID_BTC, 0).unwrap();

        supply_and_demand
            .apply_resolved_input_contribution(
                &params.inputs[0],
                0,
                &ResolvedInputMaterial {
                    outpoint: OutPoint::new(
                        "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                            .parse::<Txid>()
                            .unwrap(),
                        0,
                    ),
                    secrets: TxOutSecrets::new(
                        AssetId::LIQUID_BTC,
                        AssetBlindingFactor::zero(),
                        1,
                        ValueBlindingFactor::zero(),
                    ),
                },
            )
            .unwrap();

        assert_eq!(supply_and_demand.pick_largest_deficit_asset(), None);
    }

    #[test]
    fn validate_demand_after_resolution_run() {
        let params = RuntimeParams {
            inputs: vec![InputSchema::new("issuer").with_issuance(InputIssuance {
                kind: InputIssuanceKind::New,
                asset_amount_sat: 0,
                token_amount_sat: 0,
                entropy: [7; 32],
            })],
            outputs: vec![OutputSchema {
                id: "issued-output".to_owned(),
                amount_sat: 5,
                lock: LockVariant::Wallet,
                asset: AssetVariant::NewIssuanceAsset { input_index: 0 },
                blinder: BlinderVariant::Wallet,
            }],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let mut supply_and_demand =
            SupplyAndDemand::try_from_runtime_params(&params, AssetId::LIQUID_BTC, 0).unwrap();

        assert!(supply_and_demand
            .validate_demand_after_resolution()
            .is_err());

        supply_and_demand
            .apply_resolved_input_contribution(
                &params.inputs[0],
                0,
                &ResolvedInputMaterial {
                    outpoint: OutPoint::new(
                        "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                            .parse::<Txid>()
                            .unwrap(),
                        0,
                    ),
                    secrets: TxOutSecrets::new(
                        AssetId::LIQUID_BTC,
                        AssetBlindingFactor::zero(),
                        1,
                        ValueBlindingFactor::zero(),
                    ),
                },
            )
            .unwrap();

        assert!(supply_and_demand.validate_demand_after_resolution().is_ok());
    }

    #[test]
    fn residuals_return_change_and_funding_errors() {
        let asset_a = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let asset_b = "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a471"
            .parse::<AssetId>()
            .unwrap();
        let mut supply = AssetBalances::new();
        let mut demand = AssetBalances::new();

        supply.insert(asset_a, 15);
        supply.insert(asset_b, 4);
        demand.insert(asset_a, 10);
        demand.insert(asset_b, 4);

        let residuals = SupplyAndDemand::residuals_or_funding_error(&supply, &demand, 0).unwrap();
        assert_eq!(residuals.get(&asset_a), Some(&5));
        assert_eq!(residuals.get(&asset_b), None);

        demand.insert(asset_b, 5);
        assert!(SupplyAndDemand::residuals_or_funding_error(&supply, &demand, 0).is_err());
    }

    #[test]
    fn aggregate_issuance_supply_counts_issued_assets() {
        let params = RuntimeParams {
            inputs: vec![InputSchema::new("issuer").with_issuance(InputIssuance {
                kind: InputIssuanceKind::New,
                asset_amount_sat: 3,
                token_amount_sat: 2,
                entropy: [7; 32],
            })],
            outputs: vec![],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let input_outpoint = OutPoint::new(
            "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                .parse::<Txid>()
                .unwrap(),
            0,
        );
        let mut pst = PartiallySignedTransaction::new_v2();
        pst.add_input(Input::from_prevout(input_outpoint));

        let issuance_supply = SupplyAndDemand::aggregate_issuance_supply(&pst, &params).unwrap();
        let entropy =
            calculate_issuance_entropy(input_outpoint, params.inputs[0].issuance.as_ref().unwrap());

        assert_eq!(
            issuance_supply.get(&AssetId::from_entropy(entropy)),
            Some(&3)
        );
        assert_eq!(
            issuance_supply.get(&issuance_token_from_entropy_for_unblinded_issuance(entropy)),
            Some(&2)
        );
    }

    #[test]
    fn aggregate_input_supply_combines_prevouts_and_issuance() {
        let base_asset = "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let params = RuntimeParams {
            inputs: vec![InputSchema::new("issuer").with_issuance(InputIssuance {
                kind: InputIssuanceKind::New,
                asset_amount_sat: 3,
                token_amount_sat: 0,
                entropy: [7; 32],
            })],
            outputs: vec![],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };
        let input_outpoint = OutPoint::new(
            "0000660186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                .parse::<Txid>()
                .unwrap(),
            0,
        );
        let mut pst = PartiallySignedTransaction::new_v2();
        let mut input = Input::from_prevout(input_outpoint);
        input.asset = Some(base_asset);
        input.amount = Some(7);
        pst.add_input(input);

        let supply = SupplyAndDemand::aggregate_input_supply(&pst, &params).unwrap();

        assert_eq!(supply.get(&base_asset), Some(&7));
        assert_eq!(supply.values().copied().sum::<u64>(), 10);
    }

    #[test]
    fn aggregate_output_demand_sums_outputs_by_asset() {
        let asset_a = "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let asset_b = "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a471"
            .parse::<AssetId>()
            .unwrap();
        let mut pst = PartiallySignedTransaction::new_v2();
        pst.add_output(Output::new_explicit(Script::new(), 2, asset_a, None));
        pst.add_output(Output::new_explicit(Script::new(), 3, asset_a, None));
        pst.add_output(Output::new_explicit(Script::new(), 5, asset_b, None));

        let demand = SupplyAndDemand::aggregate_output_demand(&pst).unwrap();

        assert_eq!(demand.get(&asset_a), Some(&5));
        assert_eq!(demand.get(&asset_b), Some(&5));
    }

    #[test]
    fn exact_asset_conservation_detects_match_and_mismatch() {
        let asset_id = "0000560186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            .parse::<AssetId>()
            .unwrap();
        let input_outpoint = OutPoint::new(
            "0000660186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                .parse::<Txid>()
                .unwrap(),
            0,
        );
        let params = RuntimeParams {
            inputs: vec![InputSchema::new("declared")],
            outputs: vec![],
            fee_rate_sat_kvb: None,
            lock_time: None,
        };

        let mut balanced = PartiallySignedTransaction::new_v2();
        let mut balanced_input = Input::from_prevout(input_outpoint);
        balanced_input.asset = Some(asset_id);
        balanced_input.amount = Some(12);
        balanced.add_input(balanced_input);
        balanced.add_output(Output::new_explicit(Script::new(), 12, asset_id, None));
        assert!(SupplyAndDemand::assert_exact_asset_conservation(&balanced, &params).is_ok());

        let mut imbalanced = PartiallySignedTransaction::new_v2();
        let mut imbalanced_input = Input::from_prevout(input_outpoint);
        imbalanced_input.asset = Some(asset_id);
        imbalanced_input.amount = Some(12);
        imbalanced.add_input(imbalanced_input);
        imbalanced.add_output(Output::new_explicit(Script::new(), 11, asset_id, None));
        assert!(SupplyAndDemand::assert_exact_asset_conservation(&imbalanced, &params).is_err());
    }
}
