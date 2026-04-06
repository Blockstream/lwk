use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{AssetVariant, RuntimeParams};
use crate::wallet_abi::tx_resolution::utils::add_balance;

use std::collections::BTreeMap;

use lwk_wollet::elements::AssetId;

pub(crate) type AssetBalances = BTreeMap<AssetId, u64>;

pub(crate) struct SupplyAndDemand {
    demand_by_asset: AssetBalances,
    supply_by_asset: AssetBalances,
}

impl SupplyAndDemand {
    pub(crate) fn try_from_runtime_params(
        params: &RuntimeParams,
        policy_asset: AssetId,
        fee_target_sat: u64,
    ) -> Result<Self, WalletAbiError> {
        let mut demand_by_asset = AssetBalances::new();

        for output in &params.outputs {
            if let AssetVariant::AssetId { asset_id } = output.asset {
                add_balance(&mut demand_by_asset, asset_id, output.amount_sat)?;
            }
        }

        add_balance(&mut demand_by_asset, policy_asset, fee_target_sat)?;

        Ok(Self {
            demand_by_asset,
            supply_by_asset: AssetBalances::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SupplyAndDemand;
    use crate::wallet_abi::schema::{
        AssetVariant, BlinderVariant, LockVariant, OutputSchema, RuntimeParams,
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
    }
}
