use std::convert::TryInto;

use bitcoin::hashes::core::fmt::Formatter;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

//TODO remove this, `fn needs` could return BTreeMap<String, u64> instead
#[derive(Debug)]
pub struct AssetValue {
    pub asset: String,
    pub satoshi: u64,
}

impl AssetValue {
    pub fn new(asset: String, satoshi: u64) -> Self {
        AssetValue { asset, satoshi }
    }
}

pub type AssetId = [u8; 32]; // TODO use elements::issuance::AssetId

#[derive(Serialize, Deserialize)]
pub struct Unblinded {
    pub asset: AssetId,
    pub abf: [u8; 32],
    pub vbf: [u8; 32],
    pub value: u64,
}

impl Unblinded {
    pub fn asset_hex(&self) -> String {
        asset_to_hex(&self.asset)
    }
}

impl Debug for Unblinded {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.asset_hex(), self.value)
    }
}

pub fn asset_to_bin(asset: &str) -> Result<AssetId, crate::error::Error> {
    let mut bytes = hex::decode(asset)?;
    bytes.reverse();
    let asset: AssetId = (&bytes[..]).try_into()?;
    Ok(asset)
}

pub fn asset_to_hex(asset: &[u8]) -> String {
    let mut asset = asset.to_vec();
    asset.reverse();
    hex::encode(asset)
}

#[cfg(test)]
mod tests {
    use crate::asset::{asset_to_bin, asset_to_hex};

    #[test]
    fn test_asset_roundtrip() {
        let expected = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let result = asset_to_hex(&asset_to_bin(expected).unwrap());
        assert_eq!(expected, &result);
    }
}
