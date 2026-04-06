use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{AssetVariant, RuntimeParams};
use crate::wallet_abi::tx_resolution::utils::{add_balance, validate_output_input_index};

use std::collections::{BTreeMap, HashMap};

use lwk_wollet::elements::AssetId;
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
}

#[cfg(test)]
mod tests {
    use super::{DeferredDemandKind, SupplyAndDemand};
    use crate::wallet_abi::schema::{
        AssetVariant, BlinderVariant, InputSchema, LockVariant, OutputSchema, RuntimeParams,
    };

    use lwk_wollet::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use lwk_wollet::elements::{AssetId, OutPoint, Transaction, TxOut, TxOutSecrets, Txid};
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
}
