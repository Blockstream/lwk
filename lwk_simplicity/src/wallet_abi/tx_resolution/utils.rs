use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{InputIssuance, InputIssuanceKind};
use crate::wallet_abi::tx_resolution::supply_and_demand::IssuanceReferenceKind;

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use lwk_wollet::elements::{AssetId, ContractHash, OutPoint};
use lwk_wollet::hashes::sha256::Midstate;
use lwk_wollet::hashes::Hash;

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

pub(crate) fn validate_output_input_index(
    output_id: &str,
    input_index: u32,
    input_count: usize,
) -> Result<usize, WalletAbiError> {
    let idx = usize::try_from(input_index)?;

    if idx >= input_count {
        return Err(WalletAbiError::InvalidRequest(format!(
            "output '{output_id}' references missing input_index {input_index}"
        )));
    }

    Ok(idx)
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

pub(super) fn issuance_reference_asset_id(
    kind: IssuanceReferenceKind,
    issuance: &InputIssuance,
    outpoint: OutPoint,
    invalid_kind_error: impl FnOnce() -> WalletAbiError,
) -> Result<AssetId, WalletAbiError> {
    let issuance_entropy = calculate_issuance_entropy(outpoint, issuance);

    match (kind, &issuance.kind) {
        (IssuanceReferenceKind::NewAsset, InputIssuanceKind::New)
        | (IssuanceReferenceKind::ReissueAsset, InputIssuanceKind::Reissue) => {
            Ok(AssetId::from_entropy(issuance_entropy))
        }
        (IssuanceReferenceKind::NewToken, InputIssuanceKind::New) => Ok(
            issuance_token_from_entropy_for_unblinded_issuance(issuance_entropy),
        ),
        (IssuanceReferenceKind::NewAsset, InputIssuanceKind::Reissue)
        | (IssuanceReferenceKind::NewToken, InputIssuanceKind::Reissue)
        | (IssuanceReferenceKind::ReissueAsset, InputIssuanceKind::New) => {
            Err(invalid_kind_error())
        }
    }
}
