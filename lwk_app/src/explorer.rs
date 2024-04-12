use crate::Error;
use lwk_wollet::elements::encode::deserialize;
use lwk_wollet::elements::hex::FromHex;
use lwk_wollet::elements::{AssetId, Transaction, Txid};
use lwk_wollet::Contract;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct OutPointS {
    pub txid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryData {
    pub contract: Contract,
    pub issuance_txin: OutPointS,
}

pub fn get_registry_data(registry_url: &str, asset: &AssetId) -> Result<RegistryData, Error> {
    let url = format!("{registry_url}{asset}");
    let data: RegistryData = reqwest::blocking::get(url)?.json()?;
    Ok(data)
}

pub fn get_tx(explorer_url: &str, txid: &Txid) -> Result<Transaction, Error> {
    let url = format!("{explorer_url}api/tx/{txid}/hex");
    let tx_hex = reqwest::blocking::get(url)?.text()?;
    let bytes = Vec::<u8>::from_hex(&tx_hex)?;
    let tx = deserialize(&bytes)?;
    Ok(tx)
}
