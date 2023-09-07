use elements::{OutPoint, Script, Transaction, TxOutSecrets};
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
pub struct TransactionDetails {
    pub transaction: Transaction,
    pub txid: String,
    pub height: Option<u32>,
}

impl TransactionDetails {
    pub fn new(transaction: Transaction, height: Option<u32>) -> TransactionDetails {
        let txid = transaction.txid().to_string();
        TransactionDetails {
            transaction,
            txid,
            height,
        }
    }

    pub fn hex(&self) -> String {
        hex::encode(elements::encode::serialize(&self.transaction))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct GetTransactionsOpt {
    pub first: usize,
    pub count: usize,
    pub num_confs: Option<usize>,
}

#[cfg(test)]
mod tests {
    use elements::hex::ToHex;
    use std::str::FromStr;

    #[test]
    fn test_asset_roundtrip() {
        let hex = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let asset = elements::issuance::AssetId::from_str(&hex).unwrap();
        assert_eq!(asset.to_hex(), hex);
    }
}
