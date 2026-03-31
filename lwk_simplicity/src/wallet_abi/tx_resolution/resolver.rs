use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{
    InputIssuanceKind, InputSchema, RuntimeParams, WalletProviderMeta, WalletRequestSession,
};
use crate::wallet_abi::tx_resolution::input_material::{
    InputMaterialResolver, ResolvedInputMaterial,
};
use crate::wallet_abi::tx_resolution::resolution_artifact::ResolutionArtifacts;
use crate::wallet_abi::tx_resolution::supply_and_demand::SupplyAndDemand;

use std::sync::Arc;

use lwk_wollet::elements::pset::{Input, PartiallySignedTransaction};
use lwk_wollet::elements::secp256k1_zkp;
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

    pub(crate) fn wallet_provider(&self) -> &WalletProvider {
        self.wallet_provider
    }

    pub(crate) fn wallet_snapshot(&self) -> &Arc<[ExternalUtxo]> {
        &self.wallet_request_session.spendable_utxos
    }

    pub(crate) async fn resolve_request(
        &self,
        runtime_params: &RuntimeParams,
        mut pst: PartiallySignedTransaction,
    ) -> Result<(PartiallySignedTransaction, ResolutionArtifacts), WalletAbiError> {
        let mut artifacts: ResolutionArtifacts = ResolutionArtifacts::new();
        let mut supply_and_demand: SupplyAndDemand = SupplyAndDemand::try_from_runtime_params(
            runtime_params,
            *self.wallet_request_session.network.policy_asset(),
            self.fee_target_sat,
        )?;

        let mut input_material_resolver = InputMaterialResolver::new(self);

        for (input_index, input) in runtime_params.inputs.iter().enumerate() {
            let material = input_material_resolver
                .resolve_declared_input_material(input)
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

        todo!()
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
}
