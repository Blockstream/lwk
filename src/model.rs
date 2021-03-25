use crate::AssetId;
use bitcoin::Script;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use bitcoin::hashes::core::fmt::Formatter;
use bitcoin::util::bip32::DerivationPath;
use elements::OutPoint;
use std::convert::TryInto;
use std::fmt::Display;

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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AddressAmount {
    pub address: String, // could be bitcoin or elements
    pub satoshi: u64,
    pub asset_tag: Option<String>,
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SPVVerifyTx {
    pub txid: String,
    pub height: u32,
    pub path: String,
    pub config: crate::network::Config,
    pub encryption_key: String,
    pub tor_proxy: Option<String>,
    pub headers_to_download: Option<usize>, // defaults to 2016, useful to set for testing
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

/// Change to the model of Settings and Pricing structs could break old versions.
/// You can't remove fields, change fields type and if you add a new field, it must be Option<T>
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Settings {
    pub unit: String,
    pub required_num_blocks: u32,
    pub altimeout: u32,
    pub pricing: Pricing,
    pub sound: bool,
}

/// see comment for struct Settings
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Pricing {
    currency: String,
    exchange: String,
}

impl Default for Settings {
    fn default() -> Self {
        let pricing = Pricing {
            currency: "USD".to_string(),
            exchange: "BITFINEX".to_string(),
        };
        Settings {
            unit: "BTC".to_string(),
            required_num_blocks: 12,
            altimeout: 600,
            pricing,
            sound: false,
        }
    }
}

impl AddressAmount {
    pub fn asset(&self) -> Option<AssetId> {
        if let Some(asset_tag) = self.asset_tag.as_ref() {
            let vec = hex::decode(asset_tag).ok();
            if let Some(mut vec) = vec {
                vec.reverse();
                return (&vec[..]).try_into().ok();
            }
        }
        None
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
