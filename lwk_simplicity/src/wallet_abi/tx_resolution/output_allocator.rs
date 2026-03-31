use crate::error::WalletAbiError;
use crate::scripts::{create_p2tr_address, load_program};
use crate::wallet_abi::schema::runtime_params::bip_0341_example_internal_key;
use crate::wallet_abi::schema::{
    resolve_arguments, AssetVariant, BlinderVariant, FinalizerSpec, InputIssuance,
    InternalKeySource, LockVariant, RuntimeParams, WalletOutputRequest, WalletOutputTemplate,
    WalletProviderMeta, WalletRequestSession,
};
use crate::wallet_abi::tx_resolution::resolution_artifact::ResolutionArtifacts;
use crate::wallet_abi::tx_resolution::supply_and_demand::{IssuanceReferenceKind, SupplyAndDemand};
use crate::wallet_abi::tx_resolution::utils::{
    issuance_reference_asset_id, validate_output_input_index,
};

use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::elements::pset::{Output, PartiallySignedTransaction};
use lwk_wollet::elements::{AssetId, OutPoint, Script};

pub(crate) struct OutputAllocator<'a, WalletProvider> {
    wallet_provider: &'a WalletProvider,
    wallet_request_session: &'a WalletRequestSession,
    next_receive_index: u32,
    next_change_index: u32,
}

impl<'a, WalletProvider> OutputAllocator<'a, WalletProvider>
where
    WalletProvider: WalletProviderMeta,
    WalletAbiError: From<WalletProvider::Error>,
{
    pub(crate) fn new(
        wallet_provider: &'a WalletProvider,
        wallet_request_session: &'a WalletRequestSession,
    ) -> Self {
        Self {
            wallet_provider,
            wallet_request_session,
            next_receive_index: 0,
            next_change_index: 0,
        }
    }

    pub(crate) fn materialize_requested_outputs(
        &mut self,
        mut pst: PartiallySignedTransaction,
        resolution_artifacts: &ResolutionArtifacts,
        params: &RuntimeParams,
        fee_target_sat: u64,
    ) -> Result<PartiallySignedTransaction, WalletAbiError> {
        for output in &params.outputs {
            let asset_id = resolve_output_asset(&output.id, &output.asset, &pst, params)?;
            let wallet_template = match output.lock {
                LockVariant::Wallet => Some(self.next_receive_template()?),
                _ => None,
            };
            let script = resolve_output_lock_script(
                self.wallet_request_session.network,
                wallet_template.as_ref(),
                &output.lock,
                &pst,
            )?;

            let blinding_key: Option<PublicKey> = match output.blinder {
                BlinderVariant::Wallet => {
                    if !matches!(output.lock, LockVariant::Wallet) {
                        return Err(WalletAbiError::InvalidRequest(format!(
                            "output '{}' uses blinder.type='wallet' but lock.type is not 'wallet'",
                            output.id
                        )));
                    }
                    Some(PublicKey::new(
                        wallet_template
                            .as_ref()
                            .ok_or_else(|| {
                                WalletAbiError::InvalidResponse(
                                    "wallet receive template missing for wallet output lock"
                                        .to_string(),
                                )
                            })?
                            .blinding_pubkey
                            .ok_or_else(|| {
                                WalletAbiError::InvalidResponse(
                                    "wallet receive template missing blinding pubkey for wallet output blinder"
                                        .to_string(),
                                )
                            })?,
                    ))
                }
                BlinderVariant::Provided { pubkey } => Some(pubkey.into()),
                BlinderVariant::Explicit => None,
            };

            pst.add_output(Output::new_explicit(
                script,
                output.amount_sat,
                asset_id,
                blinding_key,
            ));
        }

        pst.add_output(Output::new_explicit(
            Script::new(),
            fee_target_sat,
            *self.wallet_request_session.network.policy_asset(),
            None,
        ));

        let mut pst = self.append_global_change_outputs(pst, params, fee_target_sat)?;

        let Some(wallet_blinder_index) = resolution_artifacts
            .secrets()
            .iter()
            .next()
            .map(|secret| secret.0)
        else {
            return Err(WalletAbiError::InvalidRequest(
                "after the input resolution excepted secrets to be non-empty".to_string(),
            ));
        };
        for output in pst.outputs_mut().iter_mut() {
            if output.blinding_key.is_none() {
                output.blinder_index = None;
                continue;
            }

            output.blinder_index = Some(u32::try_from(*wallet_blinder_index)?);
        }

        SupplyAndDemand::assert_exact_asset_conservation(&pst, params)?;

        Ok(pst)
    }

    /// Append one blinded change output per positive residual asset.
    ///
    /// Change outputs are deterministic because `residual_by_asset` is a `BTreeMap` and therefore
    /// iterated in ascending `AssetId` order.
    fn append_global_change_outputs(
        &mut self,
        mut pst: PartiallySignedTransaction,
        runtime_params: &RuntimeParams,
        fee_target_sat: u64,
    ) -> Result<PartiallySignedTransaction, WalletAbiError> {
        let supply_by_asset = SupplyAndDemand::aggregate_input_supply(&pst, runtime_params)?;
        let demand_by_asset = SupplyAndDemand::aggregate_output_demand(&pst)?;
        let residual_by_asset = SupplyAndDemand::residuals_or_funding_error(
            &supply_by_asset,
            &demand_by_asset,
            fee_target_sat,
        )?;

        for (asset_id, residual_sat) in residual_by_asset {
            if residual_sat == 0 {
                continue;
            }

            let change_template = self.next_change_template(asset_id)?;
            let change_blinding_key = change_template.blinding_pubkey.ok_or_else(|| {
                WalletAbiError::InvalidResponse(
                    "wallet change template missing blinding pubkey for change output".to_string(),
                )
            })?;

            pst.add_output(Output::new_explicit(
                change_template.script_pubkey.clone(),
                residual_sat,
                asset_id,
                Some(PublicKey::new(change_blinding_key)),
            ));
        }

        Ok(pst)
    }

    fn next_receive_template(&mut self) -> Result<WalletOutputTemplate, WalletAbiError> {
        let request = WalletOutputRequest::Receive {
            index: self.next_receive_index,
        };
        let template = self
            .wallet_provider
            .get_wallet_output_template(self.wallet_request_session, &request)?;
        self.next_receive_index = self.next_receive_index.checked_add(1).ok_or_else(|| {
            WalletAbiError::InvalidRequest("wallet receive output ordinal overflow".to_string())
        })?;
        Ok(template)
    }

    fn next_change_template(
        &mut self,
        asset_id: AssetId,
    ) -> Result<WalletOutputTemplate, WalletAbiError> {
        let request = WalletOutputRequest::Change {
            index: self.next_change_index,
            asset_id,
        };
        let template = self
            .wallet_provider
            .get_wallet_output_template(self.wallet_request_session, &request)?;
        self.next_change_index = self.next_change_index.checked_add(1).ok_or_else(|| {
            WalletAbiError::InvalidRequest("wallet change output ordinal overflow".to_string())
        })?;
        Ok(template)
    }
}

/// Resolve output locking script from request lock variant.
///
/// - `Wallet` uses the frozen request-scoped wallet receive template.
/// - `Script` uses caller-provided script directly.
/// - `FinalizerSpec::Simf` derives taproot script from finalizer metadata.
fn resolve_output_lock_script(
    network: lwk_common::Network,
    wallet_template: Option<&WalletOutputTemplate>,
    lock: &LockVariant,
    pst: &PartiallySignedTransaction,
) -> Result<Script, WalletAbiError> {
    match lock {
        LockVariant::Wallet => Ok(wallet_template
            .ok_or_else(|| {
                WalletAbiError::InvalidResponse(
                    "wallet receive template missing for wallet output lock".to_string(),
                )
            })?
            .script_pubkey
            .clone()),
        LockVariant::Script { script } => {
            if script.is_empty() {
                return Err(WalletAbiError::InvalidRequest(
                    "lock.type='script' cannot use empty script; manual fee output injection is not supported in default runtime because fee outputs are added by runtime"
                        .to_string(),
                ));
            }
            Ok(script.clone())
        }
        LockVariant::Finalizer { finalizer } => {
            resolve_finalizer_script_pubkey(finalizer, network, pst)
        }
    }
}

/// Resolve one output `AssetVariant` into a concrete `AssetId`.
///
/// Issuance-linked variants validate issuance-kind compatibility against the referenced input.
fn resolve_output_asset(
    output_id: &str,
    variant: &AssetVariant,
    pst: &PartiallySignedTransaction,
    params: &RuntimeParams,
) -> Result<AssetId, WalletAbiError> {
    match variant {
        AssetVariant::AssetId { asset_id } => Ok(*asset_id),
        AssetVariant::NewIssuanceAsset { input_index } => {
            let (issuance, outpoint) =
                resolve_issuance_asset_context(output_id, *input_index, pst, params)?;
            issuance_reference_asset_id(IssuanceReferenceKind::NewAsset, issuance, outpoint, || {
                WalletAbiError::InvalidRequest(format!(
                        "output '{output_id}' new_issuance_asset references non-new issuance input index {input_index}"
                    ))
            })
        }
        AssetVariant::NewIssuanceToken { input_index } => {
            let (issuance, outpoint) =
                resolve_issuance_asset_context(output_id, *input_index, pst, params)?;
            issuance_reference_asset_id(IssuanceReferenceKind::NewToken, issuance, outpoint, || {
                WalletAbiError::InvalidRequest(format!(
                        "output '{output_id}' new_issuance_token references non-new issuance input index {input_index}"
                    ))
            })
        }
        AssetVariant::ReIssuanceAsset { input_index } => {
            let (issuance, outpoint) =
                resolve_issuance_asset_context(output_id, *input_index, pst, params)?;
            issuance_reference_asset_id(
                IssuanceReferenceKind::ReissueAsset,
                issuance,
                outpoint,
                || {
                    WalletAbiError::InvalidRequest(format!(
                        "output '{output_id}' re_issuance_asset references non-reissue input index {input_index}"
                    ))
                },
            )
        }
    }
}

/// Resolve the issuance context required for one issuance-derived output asset.
///
/// The returned tuple is `(issuance_metadata, prevout)`.
fn resolve_issuance_asset_context<'a>(
    output_id: &str,
    input_index: u32,
    pst: &PartiallySignedTransaction,
    params: &'a RuntimeParams,
) -> Result<(&'a InputIssuance, OutPoint), WalletAbiError> {
    let idx = validate_output_input_index(output_id, input_index, params.inputs.len())?;
    let input = params.inputs.get(idx).ok_or_else(|| {
        WalletAbiError::InvalidRequest(format!(
            "output '{output_id}' references missing input_index {input_index}"
        ))
    })?;
    let issuance = input.issuance.as_ref().ok_or_else(|| {
        WalletAbiError::InvalidRequest(format!(
            "output '{output_id}' references input {} but input '{}' has no issuance metadata",
            input_index, input.id
        ))
    })?;

    let pset_input = pst.inputs().get(idx).ok_or_else(|| {
        WalletAbiError::InvalidRequest(format!(
            "resolved PSET input index {input_index} missing while materializing output '{output_id}'"
        ))
    })?;
    let outpoint = OutPoint {
        txid: pset_input.previous_txid,
        vout: pset_input.previous_output_index,
    };
    Ok((issuance, outpoint))
}

/// Resolve script pubkey for `LockVariant::Finalizer`.
///
/// Behavior contract:
/// - `Wallet` returns `InvalidRequest` because no Simplicity lock is present.
/// - `Simf + BIP0341` resolves arguments, loads `source_simf`, then derives a
///   Taproot script pubkey from program CMR + fixed BIP-0341 key + `network`.
/// - `Simf + External` resolves arguments, loads `source_simf`, derives the
///   expected address from `key.pubkey` + `network`, and fails on
///   `key.address` mismatch.
///
pub(super) fn resolve_finalizer_script_pubkey(
    spec: &FinalizerSpec,
    network: lwk_common::Network,
    pst: &PartiallySignedTransaction,
) -> Result<Script, WalletAbiError> {
    match spec {
        FinalizerSpec::Wallet => Err(WalletAbiError::InvalidRequest(
            "misconfigured finalizer spec for non-special lock case".to_string(),
        )),
        FinalizerSpec::Simf {
            source_simf,
            internal_key,
            arguments,
            ..
        } => {
            let arguments = resolve_arguments(arguments, pst)?;
            let program = load_program(source_simf, arguments)?;
            let script = match internal_key {
                InternalKeySource::Bip0341 => create_p2tr_address(
                    program.commit().cmr(),
                    &bip_0341_example_internal_key(),
                    network.address_params(),
                )
                .script_pubkey(),
                InternalKeySource::External { key } => {
                    let expected_address = create_p2tr_address(
                        program.commit().cmr(),
                        &key.get_x_only_pubkey(),
                        network.address_params(),
                    );
                    if key.address != expected_address {
                        return Err(WalletAbiError::InvalidRequest(format!(
                            "external internal key mismatch: expected address {expected_address}, got {}",
                            key.address
                        )));
                    }
                    key.address.script_pubkey()
                }
            };

            Ok(script)
        }
    }
}
