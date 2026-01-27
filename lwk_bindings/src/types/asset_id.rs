use std::{fmt::Display, str::FromStr};

use elements::hex::ToHex;

use crate::blockdata::out_point::OutPoint;
use crate::types::{ContractHash, Hex};
use crate::UniffiCustomTypeConverter;

/// A valid asset identifier.
///
/// 32 bytes encoded as hex string.
#[derive(PartialEq, Eq, Debug, Hash, Clone, Copy, PartialOrd, Ord)]
pub struct AssetId {
    inner: elements::AssetId,
}

impl Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<elements::AssetId> for AssetId {
    fn from(inner: elements::AssetId) -> Self {
        AssetId { inner }
    }
}

impl From<AssetId> for elements::AssetId {
    fn from(value: AssetId) -> Self {
        value.inner
    }
}

uniffi::custom_type!(AssetId, String);
impl UniffiCustomTypeConverter for AssetId {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        let inner = elements::AssetId::from_str(&val)?;
        Ok(AssetId { inner })
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.inner.to_hex()
    }
}

impl AssetId {
    /// Return the inner byte-order hex representation of the asset identifier.
    pub fn inner_hex(&self) -> Hex {
        Hex::from(self.inner.into_inner().to_byte_array().to_vec())
    }
}

/// TODO: delete when AssetId is refactored as uniffi::Object.
#[uniffi::export]
pub fn asset_id_inner_hex(asset_id: AssetId) -> Hex {
    asset_id.inner_hex()
}

/// Generate the asset entropy from the issuance prevout and the contract hash.
#[uniffi::export]
pub fn generate_asset_entropy(outpoint: &OutPoint, contract_hash: &ContractHash) -> Hex {
    let midstate = elements::AssetId::generate_asset_entropy(outpoint.into(), contract_hash.into());
    Hex::from(midstate.to_byte_array().to_vec())
}

/// Compute the asset ID from an issuance outpoint and contract hash.
#[uniffi::export]
pub fn asset_id_from_issuance(outpoint: &OutPoint, contract_hash: &ContractHash) -> AssetId {
    let entropy = elements::AssetId::generate_asset_entropy(outpoint.into(), contract_hash.into());
    elements::AssetId::from_entropy(entropy).into()
}

/// Compute the reissuance token ID from an issuance outpoint and contract hash.
#[uniffi::export]
pub fn reissuance_token_from_issuance(
    outpoint: &OutPoint,
    contract_hash: &ContractHash,
    is_confidential: bool,
) -> AssetId {
    let entropy = elements::AssetId::generate_asset_entropy(outpoint.into(), contract_hash.into());
    elements::AssetId::reissuance_token_from_entropy(entropy, is_confidential).into()
}

#[cfg(test)]
mod tests {
    use super::AssetId;
    use crate::{ContractHash, OutPoint, UniffiCustomTypeConverter};

    #[test]
    fn asset_id() {
        let elements_asset_id = elements::AssetId::default();
        let asset_id: AssetId = elements_asset_id.into();
        assert_eq!(
            <AssetId as UniffiCustomTypeConverter>::into_custom(
                UniffiCustomTypeConverter::from_custom(asset_id)
            )
            .unwrap(),
            asset_id
        );
    }

    /// Test against a real on-chain issuance on Liquid testnet:
    /// txid: d41479844f8aa2182fa46392d41abf9626dee16ebb82156b105c1b47ff94a9f9
    #[test]
    fn test_asset_id_from_issuance() {
        let outpoint = OutPoint::new(
            "[elements]78b3e3232680f21f4be8c055a4fdb2edf4681bd6c0ae40edeca51331839106b4:1",
        )
        .unwrap();
        let contract_hash = ContractHash::from_hex(
            "a92d0f0f0a090c09b7970ce43a12448f55c1cc00325a6a8547d57d69f52378ec",
        )
        .unwrap();

        let asset_id = super::asset_id_from_issuance(&outpoint, &contract_hash);

        let expected: AssetId = UniffiCustomTypeConverter::into_custom(
            "ccafe2eceac041673d79234ef74b31dca811555284a84f526042dfe8114483b6".to_string(),
        )
        .unwrap();
        assert_eq!(asset_id, expected);
    }

    /// Test against a real on-chain issuance on Liquid testnet:
    /// txid: d41479844f8aa2182fa46392d41abf9626dee16ebb82156b105c1b47ff94a9f9
    #[test]
    fn test_reissuance_token_from_issuance() {
        let outpoint = OutPoint::new(
            "[elements]78b3e3232680f21f4be8c055a4fdb2edf4681bd6c0ae40edeca51331839106b4:1",
        )
        .unwrap();
        let contract_hash = ContractHash::from_hex(
            "a92d0f0f0a090c09b7970ce43a12448f55c1cc00325a6a8547d57d69f52378ec",
        )
        .unwrap();

        let token_id = super::reissuance_token_from_issuance(&outpoint, &contract_hash, false);

        let expected: AssetId = UniffiCustomTypeConverter::into_custom(
            "4923a84921dcb4836243142ea5fd158d2f0602ce9fc384631ebe64504da3160e".to_string(),
        )
        .unwrap();
        assert_eq!(token_id, expected);
    }
}
