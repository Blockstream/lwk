use crate::descriptor::Chain;
use crate::elements::{Address, AssetId, OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::pset_create::validate_address;
use crate::secp256k1::PublicKey;
use crate::store::Timestamp;
use crate::{ElementsNetwork, Error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletTxOut {
    pub outpoint: OutPoint,
    pub script_pubkey: Script,
    pub height: Option<u32>,
    pub unblinded: TxOutSecrets,
    pub wildcard_index: u32,
    pub ext_int: Chain,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WalletTx {
    pub tx: Transaction,
    pub txid: Txid,
    pub height: Option<u32>,
    pub balance: HashMap<AssetId, i64>,
    pub fee: u64,
    pub type_: String,
    pub timestamp: Option<Timestamp>,
    pub inputs: Vec<Option<WalletTxOut>>,
    pub outputs: Vec<Option<WalletTxOut>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Recipient {
    pub satoshi: u64,
    pub script_pubkey: Script,
    pub blinding_pubkey: Option<PublicKey>,
    pub asset: AssetId,
}

impl Recipient {
    pub fn from_address(satoshi: u64, address: &Address, asset: AssetId) -> Self {
        Self {
            satoshi,
            script_pubkey: address.script_pubkey(),
            blinding_pubkey: address.blinding_pubkey,
            asset,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UnvalidatedRecipient {
    /// The amount to send in satoshi
    pub satoshi: u64,

    /// The address to send to
    ///
    /// If "burn", the output will be burned
    pub address: String,

    /// The asset to send
    ///
    /// If empty, the policy asset
    pub asset: String,
}

impl UnvalidatedRecipient {
    pub fn lbtc(address: String, satoshi: u64) -> Self {
        UnvalidatedRecipient {
            address,
            satoshi,
            asset: "".to_string(),
        }
    }
    pub fn burn(asset: String, satoshi: u64) -> Self {
        UnvalidatedRecipient {
            address: "burn".to_string(),
            satoshi,
            asset: asset.to_string(),
        }
    }
}

impl TryFrom<String> for UnvalidatedRecipient {
    type Error = crate::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let pieces: Vec<_> = value.split(':').collect();
        if pieces.len() != 3 {
            // TODO make specific error
            return Err(Error::Generic(format!(
                r#"Invalid number of elements in string "{}", should be "address:satoshi:assetid"#,
                value,
            )));
        }
        Ok(UnvalidatedRecipient {
            satoshi: pieces[1].parse()?,
            address: pieces[0].to_string(),
            asset: pieces[2].to_string(),
        })
    }
}

impl UnvalidatedRecipient {
    fn validate_asset(&self, network: ElementsNetwork) -> Result<AssetId, Error> {
        if self.asset.is_empty() {
            Ok(network.policy_asset())
        } else {
            Ok(AssetId::from_str(&self.asset)?)
        }
    }

    fn validate_satoshi(&self) -> Result<u64, Error> {
        if self.satoshi == 0 {
            return Err(Error::InvalidAmount);
        }
        Ok(self.satoshi)
    }

    pub fn validate(&self, network: ElementsNetwork) -> Result<Recipient, Error> {
        let satoshi = self.validate_satoshi()?;
        let asset = self.validate_asset(network)?;
        if self.address == "burn" {
            let burn_script = Script::new_op_return(&[]);
            Ok(Recipient {
                satoshi,
                script_pubkey: burn_script,
                blinding_pubkey: None,
                asset,
            })
        } else {
            let address = validate_address(&self.address, network)?;
            Ok(Recipient::from_address(self.satoshi, &address, asset))
        }
    }
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

pub(crate) struct DisplayTxOutSecrets<'a>(&'a TxOutSecrets);
impl<'a> std::fmt::Display for DisplayTxOutSecrets<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{},{},{},{}",
            self.0.value, self.0.asset, self.0.value_bf, self.0.asset_bf
        )
    }
}

pub(crate) struct DisplayWalletTxInputOutputs<'a>(&'a WalletTx);
impl<'a> std::fmt::Display for DisplayWalletTxInputOutputs<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut first = true;

        for input in self.0.inputs.iter() {
            if let Some(input) = input.as_ref() {
                if !first {
                    write!(f, ",")?;
                }
                write!(f, "{}", DisplayTxOutSecrets(&input.unblinded))?;
                first = false;
            }
        }

        for output in self.0.outputs.iter() {
            if let Some(output) = output.as_ref() {
                if !first {
                    write!(f, ",")?;
                }
                write!(f, "{}", DisplayTxOutSecrets(&output.unblinded))?;
                first = false;
            }
        }
        Ok(())
    }
}

impl WalletTx {
    pub fn unblinded_url(&self, explorer_url: &str) -> String {
        format!(
            "{}tx/{}#blinded={}",
            explorer_url,
            &self.tx.txid(),
            DisplayWalletTxInputOutputs(self)
        )
    }
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
