use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{AssetVariant, RuntimeParams};
use crate::wallet_abi::tx_resolution::utils::{add_balance, validate_output_input_index};

use std::collections::{BTreeMap, HashMap};

use lwk_wollet::elements::AssetId;

pub(crate) type AssetBalances = BTreeMap<AssetId, u64>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeferredDemandKind {
    NewIssuanceAsset,
    NewIssuanceToken,
    ReissueAsset,
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
}

#[cfg(test)]
mod tests {
    use super::{DeferredDemandKind, SupplyAndDemand};
    use crate::wallet_abi::schema::{
        AssetVariant, BlinderVariant, InputSchema, LockVariant, OutputSchema, RuntimeParams,
    };

    use lwk_wollet::elements::AssetId;

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
}
