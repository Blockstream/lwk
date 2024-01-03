use crate::types::{Hex, Txid};

#[derive(uniffi::Record)]
pub struct Tx {
    pub txid: Txid,
    pub inputs: Vec<Option<TxIn>>,
    pub outputs: Vec<Option<TxOut>>,
}

#[derive(uniffi::Record)]
pub struct TxIn {
    pub prevout_txid: Txid,
    pub prevout_vout: u32,
    pub value: u64,
}

#[derive(uniffi::Record)]
pub struct TxOut {
    pub script_pubkey: Hex,
    pub value: u64,
}
