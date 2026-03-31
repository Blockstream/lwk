use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{AssetVariant, InputSchema, RuntimeParams};
use crate::wallet_abi::tx_resolution::utils::{
    add_balance, calculate_issuance_entropy, issuance_reference_asset_id,
    issuance_token_from_entropy_for_unblinded_issuance, validate_output_input_index,
};

use std::collections::{BTreeMap, HashMap};

use crate::wallet_abi::tx_resolution::input_material::ResolvedInputMaterial;
use lwk_wollet::elements::AssetId;

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
}
