use bitcoin::hashes::core::fmt::Formatter;
use bitcoin::hashes::hex::ToHex;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize)]
pub struct Unblinded {
    pub asset: elements::issuance::AssetId,
    pub assetblinder: [u8; 32],
    pub valueblinder: [u8; 32],
    pub value: u64,
}

impl Debug for Unblinded {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.asset.to_hex(), self.value)
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::hashes::hex::{FromHex, ToHex};

    #[test]
    fn test_asset_roundtrip() {
        let hex = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let asset = elements::issuance::AssetId::from_hex(&hex).unwrap();
        assert_eq!(asset.to_hex(), hex);
    }
}
