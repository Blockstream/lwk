use crate::error::WalletAbiError;
use crate::wallet_abi::schema::{InputIssuance, InputIssuanceKind};

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
            let value = entry.get_mut();
            *value = value.checked_add(amount_sat).ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "asset amount overflow while aggregating balances for {asset_id}"
                ))
            })?;
        }
    }

    Ok(())
}

/// Compute issuance entropy from input outpoint and issuance kind.
pub(super) fn calculate_issuance_entropy(outpoint: OutPoint, issuance: &InputIssuance) -> Midstate {
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
pub(super) fn issuance_token_from_entropy_for_unblinded_issuance(
    issuance_entropy: Midstate,
) -> AssetId {
    AssetId::reissuance_token_from_entropy(issuance_entropy, false)
}

/// Validate issuance-linked output references in one place so every caller
/// rejects the same out-of-bounds `input_index` before dereferencing inputs.
pub(crate) fn validate_output_input_index(
    output_id: &str,
    input_index: u32,
    input_count: usize,
) -> Result<usize, WalletAbiError> {
    let index = usize::try_from(input_index)?;

    if index >= input_count {
        return Err(WalletAbiError::InvalidRequest(format!(
            "output '{output_id}' references missing input_index {input_index}"
        )));
    }

    Ok(index)
}

#[derive(Clone, Copy)]
pub(super) enum IssuanceReferenceKind {
    NewAsset,
    NewToken,
    ReissueAsset,
}

/// Resolve the concrete asset id behind an issuance-linked reference while
/// enforcing that the requested reference kind matches the input issuance kind.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::WalletAbiError;
    use crate::wallet_abi::schema::{InputIssuance, InputIssuanceKind};

    use std::collections::BTreeMap;

    use lwk_wollet::elements::{AssetId, ContractHash, OutPoint, Txid};
    use lwk_wollet::hashes::sha256::Midstate;
    use lwk_wollet::hashes::Hash;

    #[test]
    fn add_balance_accumulates_amounts() {
        let mut balances = BTreeMap::new();

        add_balance(&mut balances, AssetId::LIQUID_BTC, 2).unwrap();
        add_balance(&mut balances, AssetId::LIQUID_BTC, 3).unwrap();

        assert_eq!(balances.get(&AssetId::LIQUID_BTC), Some(&5));

        let error = add_balance(&mut balances, AssetId::LIQUID_BTC, u64::MAX).unwrap_err();
        assert!(matches!(error, WalletAbiError::InvalidRequest(message)
                if message.contains("asset amount overflow")));
    }

    #[test]
    fn calculate_issuance_entropy_uses_contract_hash() {
        let outpoint = OutPoint::new(
            "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                .parse::<Txid>()
                .unwrap(),
            1,
        );
        let issuance = InputIssuance {
            kind: InputIssuanceKind::New,
            asset_amount_sat: 5,
            token_amount_sat: 3,
            entropy: [7; 32],
        };
        assert_eq!(
            calculate_issuance_entropy(outpoint, &issuance),
            AssetId::generate_asset_entropy(outpoint, ContractHash::from_byte_array([7; 32]))
        );

        let issuance = InputIssuance {
            kind: InputIssuanceKind::Reissue,
            asset_amount_sat: 8,
            token_amount_sat: 0,
            entropy: [9; 32],
        };
        assert_eq!(
            calculate_issuance_entropy(outpoint, &issuance),
            Midstate::from_byte_array([9; 32])
        );
    }

    #[test]
    fn issuance_reference_asset_id_derives_new_asset() {
        let outpoint = OutPoint::new(
            "0000460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
                .parse::<Txid>()
                .unwrap(),
            0,
        );
        let issuance = InputIssuance {
            kind: InputIssuanceKind::New,
            asset_amount_sat: 7,
            token_amount_sat: 2,
            entropy: [5; 32],
        };

        assert_eq!(
            issuance_reference_asset_id(
                IssuanceReferenceKind::NewAsset,
                &issuance,
                outpoint,
                || WalletAbiError::InvalidRequest("mismatch".to_owned()),
            )
            .unwrap(),
            AssetId::from_entropy(calculate_issuance_entropy(outpoint, &issuance))
        );

        let issuance = InputIssuance {
            kind: InputIssuanceKind::New,
            asset_amount_sat: 7,
            token_amount_sat: 2,
            entropy: [6; 32],
        };
        assert_eq!(
            issuance_reference_asset_id(
                IssuanceReferenceKind::NewToken,
                &issuance,
                outpoint,
                || WalletAbiError::InvalidRequest("mismatch".to_owned()),
            )
            .unwrap(),
            issuance_token_from_entropy_for_unblinded_issuance(calculate_issuance_entropy(
                outpoint, &issuance,
            ))
        );

        let issuance = InputIssuance {
            kind: InputIssuanceKind::New,
            asset_amount_sat: 1,
            token_amount_sat: 1,
            entropy: [7; 32],
        };
        let error = issuance_reference_asset_id(
            IssuanceReferenceKind::ReissueAsset,
            &issuance,
            outpoint,
            || WalletAbiError::InvalidRequest("mismatch".to_owned()),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            WalletAbiError::InvalidRequest(message) if message == "mismatch"
        ));
    }
}
