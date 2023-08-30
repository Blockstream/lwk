use crate::error::Error;
use crate::network::ElementsNetwork;

use elements::Script;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use elements::bitcoin::hashes::hex::FromHex;
use elements::OutPoint;
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
    pub unblinded: elements::TxOutSecrets,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionDetails {
    pub transaction: elements::Transaction,
    pub txid: String,
    pub balances: HashMap<elements::issuance::AssetId, i64>,
    pub fee: u64,
    pub height: Option<u32>,
}

impl TransactionDetails {
    pub fn new(
        transaction: elements::Transaction,
        balances: HashMap<elements::issuance::AssetId, i64>,
        fee: u64,
        height: Option<u32>,
    ) -> TransactionDetails {
        let txid = transaction.txid().to_string();
        TransactionDetails {
            transaction,
            txid,
            balances,
            fee,
            height,
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
    pub fn new(
        address: &str,
        satoshi: u64,
        asset: &str,
        network: ElementsNetwork,
    ) -> Result<Self, Error> {
        let address = elements::Address::parse_with_params(address, &network.address_params())
            .map_err(|_| Error::InvalidAddress)?;
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
    pub utxos: Option<Vec<UnblindedTXO>>,
}
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct GetTransactionsOpt {
    pub first: usize,
    pub count: usize,
    pub subaccount: usize,
    pub num_confs: Option<usize>,
}

// This one is simple enough to derive a serializer
#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct FeeEstimate(pub u64);

#[cfg(test)]
mod tests {
    use elements::bitcoin::hashes::hex::{FromHex, ToHex};

    #[test]
    fn test_asset_roundtrip() {
        let hex = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let asset = elements::issuance::AssetId::from_hex(&hex).unwrap();
        assert_eq!(asset.to_hex(), hex);
    }
}
