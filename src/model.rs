use crate::error::Error;

use elements::Script;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use bitcoin::hashes::core::fmt::Formatter;
use bitcoin::hashes::hex::FromHex;
use elements::OutPoint;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXO {
    pub outpoint: OutPoint,
    pub asset: elements::issuance::AssetId,
    pub satoshi: u64, // aka amount, value
    pub script_pubkey: Script,
    pub height: Option<u32>,
}

impl TXO {
    pub fn new(
        outpoint: OutPoint,
        asset: elements::issuance::AssetId,
        satoshi: u64,
        script_pubkey: Script,
        height: Option<u32>,
    ) -> TXO {
        TXO {
            outpoint,
            asset,
            satoshi,
            script_pubkey,
            height,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionDetails {
    pub transaction: elements::Transaction,
    pub txid: String,
    pub balances: HashMap<elements::issuance::AssetId, i64>,
    pub fee: u64,
    pub height: Option<u32>,
    pub spv_verified: SPVVerifyResult,
}

impl TransactionDetails {
    pub fn new(
        transaction: elements::Transaction,
        balances: HashMap<elements::issuance::AssetId, i64>,
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
pub struct Destination {
    address: elements::Address,
    satoshi: u64,
    asset: elements::issuance::AssetId,
}

impl Destination {
    pub fn new(address: &str, satoshi: u64, asset: &str) -> Result<Self, Error> {
        let address = elements::Address::from_str(address).map_err(|_| Error::InvalidAddress)?;
        let asset = elements::issuance::AssetId::from_hex(asset)?;
        Ok(Destination {
            address,
            satoshi,
            asset,
        })
    }

    pub fn address(&self) -> elements::Address {
        self.address.clone()
    }

    pub fn satoshi(&self) -> u64 {
        self.satoshi
    }

    pub fn asset(&self) -> elements::issuance::AssetId {
        self.asset
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CreateTransactionOpt {
    // TODO: chage type to hold SendAll and be valid
    pub addressees: Vec<Destination>,
    pub fee_rate: Option<u64>, // in satoshi/kbyte
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
