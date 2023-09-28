use crate::elements::{Address, AssetId, OutPoint, Script, TxOutSecrets, Txid};
use crate::secp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletTxOut {
    pub outpoint: OutPoint,
    pub script_pubkey: Script,
    pub height: Option<u32>,
    pub unblinded: TxOutSecrets,
    pub wildcard_index: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Addressee {
    pub satoshi: u64,
    pub script_pubkey: Script,
    pub blinding_pubkey: Option<PublicKey>,
    pub asset: AssetId,
}

impl Addressee {
    pub fn from_address(satoshi: u64, address: &Address, asset: AssetId) -> Self {
        Self {
            satoshi,
            script_pubkey: address.script_pubkey(),
            blinding_pubkey: address.blinding_pubkey,
            asset,
        }
    }
}

pub struct UnvalidatedAddressee<'a> {
    /// The amount to send in satoshi
    pub satoshi: u64,

    /// The address to send to
    pub address: &'a str,

    /// The asset to send
    ///
    /// If empty, the policy asset
    pub asset: &'a str,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddressResult {
    address: Address,
    index: u32,
}

impl AddressResult {
    pub fn new(address: Address, index: u32) -> Self {
        Self { address, index }
    }

    pub fn address(&self) -> &Address {
        &self.address
    }

    pub fn index(&self) -> u32 {
        self.index
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IssuanceDetails {
    pub txid: Txid,
    pub vin: u32,
    pub entropy: [u8; 32],
    pub asset: AssetId,
    pub token: AssetId,
    pub asset_amount: Option<u64>,
    pub token_amount: Option<u64>,
    pub is_reissuance: bool,
    // asset_blinder
    // token_blinder
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
