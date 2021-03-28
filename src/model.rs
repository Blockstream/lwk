use crate::error::Error;

use bitcoin::Script;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use bitcoin::hashes::core::fmt::Formatter;
use bitcoin::hashes::hex::{FromHex, ToHex};
use bitcoin::util::bip32::DerivationPath;
use elements::OutPoint;
use std::fmt::Display;
use std::str::FromStr;

pub type Balances = HashMap<String, i64>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXO {
    pub outpoint: OutPoint,
    pub asset: String,                  // Use better type
    pub satoshi: u64,                   // aka amount, value
    pub asset_blinder: Option<String>,  // FIXME
    pub amount_blinder: Option<String>, // FIXME
    pub script_pubkey: Script,
    pub height: Option<u32>,
    pub path: DerivationPath,
}

impl TXO {
    pub fn new(
        outpoint: OutPoint,
        asset: String,
        satoshi: u64,
        _asset_blinder: Option<String>,
        _amount_blinder: Option<String>,
        script_pubkey: Script,
        height: Option<u32>,
        path: DerivationPath,
    ) -> TXO {
        TXO {
            outpoint,
            asset,
            satoshi,
            asset_blinder: None,
            amount_blinder: None,
            script_pubkey,
            height,
            path,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionDetails {
    pub transaction: elements::Transaction,
    pub txid: String,
    pub balances: Balances,
    pub fee: u64,
    pub height: Option<u32>,
    pub spv_verified: SPVVerifyResult,
}

impl TransactionDetails {
    pub fn new(
        transaction: elements::Transaction,
        balances: Balances,
        fee: u64,
        height: Option<u32>,
        spv_verified: SPVVerifyResult,
    ) -> TransactionDetails {
        let txid = transaction.txid().to_string();
        TransactionDetails {
            transaction,
            txid,
            balances,
            fee,
            height,
            spv_verified,
        }
    }

    pub fn hex(&self) -> String {
        hex::encode(elements::encode::serialize(&self.transaction))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddressAmount {
    pub address: elements::Address,
    pub satoshi: u64,
    pub asset_tag: Option<String>,
}

impl AddressAmount {
    pub fn new(address: &str, satoshi: u64, asset: &str) -> Result<Self, Error> {
        let address = elements::Address::from_str(address).map_err(|_| Error::InvalidAddress)?;
        let asset = elements::issuance::AssetId::from_hex(asset)?;
        Ok(AddressAmount {
            address,
            satoshi,
            asset_tag: Some(asset.to_hex()), // TODO: asset_tag -> asset
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BlockNotification {
    //pub block_hash: bitcoin::BlockHash,
    pub block_height: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionNotification {
    pub transaction_hash: bitcoin::Txid,
}

#[derive(Debug)]
pub enum Notification {
    Block(BlockNotification),
    Transaction(TransactionNotification),
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CreateTransactionOpt {
    // TODO: chage type to hold SendAll and be valid
    pub addressees: Vec<AddressAmount>,
    pub fee_rate: Option<u64>, // in satoshi/kbyte
    // TODO: this should be in addressees
    pub send_all: Option<bool>,
    pub utxos: Option<Vec<TXO>>,
}
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct GetTransactionsOpt {
    pub first: usize,
    pub count: usize,
    pub subaccount: usize,
    pub num_confs: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SPVVerifyResult {
    InProgress,
    Verified,
    NotVerified,
    Disabled,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddressPointer {
    pub address: String,
    pub pointer: u32, // child_number in bip32 terminology
}

// This one is simple enough to derive a serializer
#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct FeeEstimate(pub u64);

impl AddressAmount {
    pub fn asset(&self) -> Option<elements::issuance::AssetId> {
        match self.asset_tag.as_ref() {
            Some(asset) => elements::issuance::AssetId::from_hex(asset).ok(),
            None => None,
        }
    }
}

impl SPVVerifyResult {
    pub fn as_i32(&self) -> i32 {
        match self {
            SPVVerifyResult::InProgress => 0,
            SPVVerifyResult::Verified => 1,
            SPVVerifyResult::NotVerified => 2,
            SPVVerifyResult::Disabled => 3,
        }
    }
}

impl Display for SPVVerifyResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SPVVerifyResult::InProgress => write!(f, "in_progress"),
            SPVVerifyResult::Verified => write!(f, "verified"),
            SPVVerifyResult::NotVerified => write!(f, "not_verified"),
            SPVVerifyResult::Disabled => write!(f, "disabled"),
        }
    }
}
