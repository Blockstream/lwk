use crate::Error;
use lwk_wollet::elements::encode::deserialize;
use lwk_wollet::elements::hex::FromHex;
use lwk_wollet::elements::{Transaction, Txid};

pub fn get_tx(esplora_api_url: &str, txid: &Txid) -> Result<Transaction, Error> {
    let url = format!("{esplora_api_url}tx/{txid}/hex");
    log::debug!("getting tx {url}");
    let tx_hex = reqwest::blocking::get(url)?.text()?;
    log::debug!("got {tx_hex}");
    let bytes = Vec::<u8>::from_hex(&tx_hex)?;
    let tx = deserialize(&bytes)?;
    Ok(tx)
}
