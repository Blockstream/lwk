use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{
    InputIssuanceKind, InputSchema, RuntimeParams, WalletProviderMeta, WalletRequestSession,
};
use crate::wallet_abi::tx_resolution::input_material::{
    InputMaterialResolver, ResolvedInputMaterial,
};
use crate::wallet_abi::tx_resolution::output_allocator::OutputAllocator;
use crate::wallet_abi::tx_resolution::resolution_artifact::ResolutionArtifacts;
use crate::wallet_abi::tx_resolution::supply_and_demand::SupplyAndDemand;

use std::collections::HashSet;
use std::sync::Arc;

use lwk_wollet::elements::pset::{Input, PartiallySignedTransaction};
use lwk_wollet::elements::{secp256k1_zkp, AssetId, OutPoint};
use lwk_wollet::secp256k1::constants::ONE;
use lwk_wollet::ExternalUtxo;

pub(super) struct Resolver<'a, WalletProvider: WalletProviderMeta> {
    wallet_request_session: &'a WalletRequestSession,
    wallet_provider: &'a WalletProvider,
    fee_target_sat: u64,
}

impl<'a, WalletProvider: WalletProviderMeta> Resolver<'a, WalletProvider>
where
    WalletAbiError: From<WalletProvider::Error>,
{
    /// Create a resolver bound to one validated wallet session and fee target
    /// so every resolution step sees the same wallet snapshot.
    pub(crate) fn new(
        wallet_request_session: &'a WalletRequestSession,
        wallet_provider: &'a WalletProvider,
        fee_target_sat: u64,
    ) -> Self {
        Self {
            wallet_request_session,
            wallet_provider,
            fee_target_sat,
        }
    }

    /// Expose the wallet provider so helper resolvers can fetch prevouts,
    /// unblindings, and wallet metadata through the same runtime backend.
    pub(crate) fn wallet_provider(&self) -> &WalletProvider {
        self.wallet_provider
    }

    /// Expose the frozen wallet snapshot so input selection remains stable for
    /// the lifetime of this transaction build.
    pub(crate) fn wallet_snapshot(&self) -> &Arc<[ExternalUtxo]> {
        &self.wallet_request_session.spendable_utxos
    }

    /// Resolve the request into a blinded PSET plus artifacts needed for later
    /// finalization and fee modeling.
    ///
    /// The flow first resolves declared inputs, then fills remaining asset
    /// deficits with auxiliary wallet inputs, and only then materializes
    /// requested outputs and change against the resulting supply state.
    pub(crate) async fn resolve_request(
        &self,
        runtime_params: &RuntimeParams,
        mut pst: PartiallySignedTransaction,
    ) -> Result<(PartiallySignedTransaction, ResolutionArtifacts), WalletAbiError> {
        let mut supply_and_demand: SupplyAndDemand = SupplyAndDemand::try_from_runtime_params(
            runtime_params,
            self.wallet_request_session.network.policy_asset(),
            self.fee_target_sat,
        )?;
        let mut artifacts: ResolutionArtifacts = ResolutionArtifacts::new();

        let mut input_material_resolver = InputMaterialResolver::new(self);

        for (input_index, input) in runtime_params.inputs.iter().enumerate() {
            let material = input_material_resolver
                .resolve_declared_input_material(input, &supply_and_demand)
                .await?;

            self.add_resolved_input_to_pset(
                &mut pst,
                &mut artifacts,
                input,
                input_index,
                &material,
            )?;

            supply_and_demand.apply_resolved_input_contribution(input, input_index, &material)?;
        }

        supply_and_demand.validate_demand_after_resolution()?;

        while let Some((target_asset, target_missing)) =
            supply_and_demand.pick_largest_deficit_asset()
        {
            let selected_indexes = self.select_auxiliary_inputs_for_asset(
                target_asset,
                target_missing,
                input_material_resolver.used_outpoints(),
            )?;

            for selected_index in selected_indexes {
                let selected: &ExternalUtxo = self.wallet_request_session.spendable_utxos.get(selected_index).ok_or_else(|| {
                    WalletAbiError::InvalidResponse(format!(
                        "wallet snapshot index {selected_index} missing while adding auxiliary input"
                    ))
                })?;

                input_material_resolver.reserve_outpoint("auxiliary", selected.outpoint)?;

                self.add_auxiliary_wallet_input(&mut pst, &mut artifacts, selected)
                    .await?;

                supply_and_demand.add_selected_wallet_to_supply(selected)?;
            }
        }

        let mut output_allocator =
            OutputAllocator::new(self.wallet_provider, self.wallet_request_session);

        let pst = output_allocator.materialize_requested_outputs(
            pst,
            &artifacts,
            runtime_params,
            self.fee_target_sat,
        )?;

        Ok((pst, artifacts))
    }

    /// Append a resolved input to the PSET and attach sequence, prevout and witness UTXO.
    fn add_resolved_input_to_pset(
        &self,
        pst: &mut PartiallySignedTransaction,
        artifacts: &mut ResolutionArtifacts,
        input: &InputSchema,
        input_index: usize,
        material: &ResolvedInputMaterial,
    ) -> Result<(), WalletAbiError> {
        let mut pset_input = Input::from_prevout(*material.outpoint());
        pset_input.sequence = Some(input.sequence);
        pset_input.witness_utxo = Some(material.tx_out().clone());
        pset_input.amount = Some(material.secrets().value);
        pset_input.asset = Some(material.secrets().asset);

        if let Some(issuance) = input.issuance.as_ref() {
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
                let mut nonce = material.secrets().asset_bf.into_inner();
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
        }

        if let Some((pubkey, key_source)) = self
            .wallet_provider
            .get_bip32_derivation_pair(material.outpoint())?
        {
            pset_input.bip32_derivation.insert(pubkey, key_source);
        }

        artifacts.collect_input(input, input_index, material)?;

        pst.add_input(pset_input);

        Ok(())
    }

    /// Append a wallet-owned funding input when declared inputs leave an asset
    /// deficit, keeping the extra funding explicit in the resulting PSET order.
    async fn add_auxiliary_wallet_input(
        &self,
        pst: &mut PartiallySignedTransaction,
        artifacts: &mut ResolutionArtifacts,
        selected: &ExternalUtxo,
    ) -> Result<(), WalletAbiError> {
        let input_index = pst.n_inputs();
        let mut pset_input = Input::from_prevout(selected.outpoint);
        pset_input.witness_utxo = Some(selected.txout.clone());
        pset_input.amount = Some(selected.unblinded.value);
        pset_input.asset = Some(selected.unblinded.asset);

        if let Some((pubkey, key_source)) = self
            .wallet_provider
            .get_bip32_derivation_pair(&selected.outpoint)?
        {
            pset_input.bip32_derivation.insert(pubkey, key_source);
        } else {
            return Err(WalletAbiError::InvalidResponse(format!(
                "missing wallet bip32 derivation pair for wallet-owned UTXO {}",
                selected.outpoint
            )));
        }

        artifacts.collect_wallet_input(selected, input_index)?;

        pst.add_input(pset_input);

        Ok(())
    }

    // TODO: later on the algorith can be replaced with bounded BnB, it is like that for simplicity reasons
    /// Choose the smallest prefix of largest same-asset wallet UTXOs that can
    /// close the current deficit, which keeps auxiliary funding deterministic
    /// and avoids pulling in unrelated assets.
    fn select_auxiliary_inputs_for_asset(
        &self,
        target_asset: AssetId,
        target_missing: u64,
        reserved_outpoints: &HashSet<OutPoint>,
    ) -> Result<Vec<usize>, WalletAbiError> {
        if target_missing == 0 {
            return Ok(Vec::new());
        }

        let mut candidate_indexes = self
            .wallet_snapshot()
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (!reserved_outpoints.contains(&candidate.outpoint)
                    && candidate.unblinded.asset == target_asset)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        candidate_indexes.sort_by(|left, right| {
            let left = &self.wallet_snapshot()[*left];
            let right = &self.wallet_snapshot()[*right];
            right
                .unblinded
                .value
                .cmp(&left.unblinded.value)
                .then_with(|| {
                    left.outpoint
                        .txid
                        .to_string()
                        .cmp(&right.outpoint.txid.to_string())
                })
                .then_with(|| left.outpoint.vout.cmp(&right.outpoint.vout))
        });

        let available_candidates = candidate_indexes.len();
        let available_total_sat = candidate_indexes.iter().try_fold(0u64, |sum, index| {
            sum.checked_add(self.wallet_snapshot()[*index].unblinded.value)
                .ok_or_else(|| {
                    WalletAbiError::InvalidRequest(format!(
                        "asset amount overflow while aggregating candidate pool for {}",
                        target_asset
                    ))
                })
        })?;

        if candidate_indexes.is_empty() || available_total_sat < target_missing {
            return Err(
                WalletAbiError::Funding(format!(
                    "insufficient wallet funds for asset {target_asset}: missing {target_missing} sat, available {available_total_sat} sat across {available_candidates} candidate UTXOs"
                ))
            );
        }

        let mut selected_indexes = Vec::new();
        let mut selected_total_sat = 0u64;

        for index in candidate_indexes {
            selected_indexes.push(index);
            selected_total_sat = selected_total_sat
                .checked_add(self.wallet_snapshot()[index].unblinded.value)
                .ok_or_else(|| {
                    WalletAbiError::InvalidRequest(format!(
                        "asset amount overflow while selecting auxiliary inputs for {}",
                        target_asset
                    ))
                })?;

            if selected_total_sat >= target_missing {
                return Ok(selected_indexes);
            }
        }

        Err(
            WalletAbiError::Funding(format!(
                "insufficient wallet funds for asset {target_asset}: missing {target_missing} sat, available {available_total_sat} sat across {available_candidates} candidate UTXOs"
            ))
        )
    }
}
