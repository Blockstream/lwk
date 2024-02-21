use crate::Error;
use lwk_wollet::{elements::{self, hex::ToHex, pset::serialize::{Deserialize, Serialize}}, hashes::hex::FromHex};
use std::str::FromStr;
use wasm_bindgen::prelude::*;


/// A Liquid transaction
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
pub struct Transaction {
    inner: elements::Transaction,
}

impl std::fmt::Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner.serialize().to_hex())
    }
}

impl From<elements::Transaction> for Transaction {
    fn from(inner: elements::Transaction) -> Self {
        Transaction { inner }
    }
}

impl From<Transaction> for elements::Transaction {
    fn from(value: Transaction) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl Transaction {
    #[wasm_bindgen(constructor)]
    pub fn new(tx_hex: &str) -> Result<Transaction, Error> {
        let bytes = Vec::<u8>::from_hex(tx_hex)?;
        let tx: elements::Transaction = elements::Transaction::deserialize(&bytes)?;
        Ok(tx.into())
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }
}

/// A valid transaction identifier.
///
/// 32 bytes encoded as hex string.
#[wasm_bindgen]
#[derive(PartialEq, Eq, Debug, Hash, Clone, Copy)]
pub struct Txid {
    inner: elements::Txid,
}

impl std::fmt::Display for Txid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<elements::Txid> for Txid {
    fn from(inner: elements::Txid) -> Self {
        Txid { inner }
    }
}

impl From<Txid> for elements::Txid {
    fn from(value: Txid) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl Txid {
    #[wasm_bindgen(constructor)]
    pub fn new(tx_id: &str) -> Result<Txid, Error> {
        Ok(elements::Txid::from_str(tx_id)?.into())
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }
}

#[cfg(test)]
mod tests {

    use wasm_bindgen_test::*;

    use crate::{Transaction, Txid};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_tx_id() {
        let expected = "HexToArray(InvalidLength(64, 2))";
        let hex = "xx";
        assert_eq!(expected, format!("{:?}", Txid::new(hex).unwrap_err()));

        let expected = "HexToArray(Conversion(InvalidChar(120)))";
        let hex = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert_eq!(expected, format!("{:?}", Txid::new(hex).unwrap_err()));

        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        assert_eq!(hex, Txid::new(hex).unwrap().to_string());
    }

    #[wasm_bindgen_test]
    async fn test_transaction() {
        let expected = "HexToBytes(InvalidChar(120))";
        let hex = "xx";
        assert_eq!(expected, format!("{:?}", Transaction::new(hex).unwrap_err()));

        let expected =
            include_str!("../../../lwk_jade/test_data/pset_to_be_signed_transaction.hex")
                .to_string();
        assert_eq!(expected, Transaction::new(&expected).unwrap().to_string());
    }
}
