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

/// Return the inner byte-order hex representation of an asset identifier.
#[uniffi::export]
pub fn asset_id_inner_hex(asset_id: AssetId) -> Hex {
    let inner: elements::AssetId = asset_id.into();
    Hex::from(inner.into_inner().to_byte_array().to_vec())
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

    use lwk_wollet::hashes::Hash;

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

    #[test]
    fn test_asset_id_from_issuance() {
        let txid_hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let vout = 0u32;
        let contract_bytes = [0u8; 32];

        let outpoint = OutPoint::new(&format!("[elements]{txid_hex}:{vout}")).unwrap();
        let contract_hash = ContractHash::from_bytes(&contract_bytes).unwrap();

        let asset_id = super::asset_id_from_issuance(&outpoint, &contract_hash);

        let el_outpoint =
            elements::OutPoint::new(txid_hex.parse::<elements::Txid>().unwrap(), vout);
        let el_contract = elements::ContractHash::from_byte_array(contract_bytes);
        let entropy = elements::AssetId::generate_asset_entropy(el_outpoint, el_contract);
        let expected: AssetId = elements::AssetId::from_entropy(entropy).into();

        assert_eq!(asset_id, expected);
    }

    #[test]
    fn test_reissuance_token_from_issuance() {
        let txid_hex = "0000000000000000000000000000000000000000000000000000000000000001";
        let vout = 0u32;
        let contract_bytes = [0u8; 32];

        let outpoint = OutPoint::new(&format!("[elements]{txid_hex}:{vout}")).unwrap();
        let contract_hash = ContractHash::from_bytes(&contract_bytes).unwrap();

        let token_id = super::reissuance_token_from_issuance(&outpoint, &contract_hash, false);

        let el_outpoint =
            elements::OutPoint::new(txid_hex.parse::<elements::Txid>().unwrap(), vout);
        let el_contract = elements::ContractHash::from_byte_array(contract_bytes);
        let entropy = elements::AssetId::generate_asset_entropy(el_outpoint, el_contract);
        let expected: AssetId =
            elements::AssetId::reissuance_token_from_entropy(entropy, false).into();

        assert_eq!(token_id, expected);
    }
}
