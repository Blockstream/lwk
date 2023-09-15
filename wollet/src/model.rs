use elements_miniscript::elements::{Address, AssetId, OutPoint, Script, TxOutSecrets};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXO {
    pub outpoint: OutPoint,
    pub script_pubkey: Script,
    pub height: Option<u32>,
}

impl TXO {
    pub fn new(outpoint: OutPoint, script_pubkey: Script, height: Option<u32>) -> TXO {
        TXO {
            outpoint,
            script_pubkey,
            height,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnblindedTXO {
    pub txo: TXO,
    pub unblinded: TxOutSecrets,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Addressee {
    pub satoshi: u64,
    pub address: Address,
    pub asset: AssetId,
}

pub struct UnvalidatedAddressee<'a> {
    pub satoshi: u64,
    pub address: &'a str,
    pub asset: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_asset_roundtrip() {
        let hex = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let asset = AssetId::from_str(hex).unwrap();
        assert_eq!(asset.to_string(), hex);
    }
}
