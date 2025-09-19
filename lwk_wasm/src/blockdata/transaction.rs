use crate::{AssetId, Error};
use lwk_wollet::{
    elements::{
        self,
        hex::ToHex,
        pset::serialize::{Deserialize, Serialize},
    },
    hashes::hex::FromHex,
};
use std::str::FromStr;
use wasm_bindgen::prelude::*;

/// A Liquid transaction
///
/// See `WalletTx` for the transaction as seen from the perspective of the wallet
/// where you can actually see unblinded amounts and tx net-balance.
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

impl AsRef<elements::Transaction> for Transaction {
    fn as_ref(&self) -> &elements::Transaction {
        &self.inner
    }
}

#[wasm_bindgen]
impl Transaction {
    /// Creates a `Transaction`
    #[wasm_bindgen(constructor)]
    pub fn new(tx_hex: &str) -> Result<Transaction, Error> {
        let bytes = Vec::<u8>::from_hex(tx_hex)?;
        let tx: elements::Transaction = elements::Transaction::deserialize(&bytes)?;
        Ok(tx.into())
    }

    pub fn txid(&self) -> Txid {
        self.inner.txid().into()
    }

    pub fn bytes(&self) -> Vec<u8> {
        elements::Transaction::serialize(&self.inner)
    }

    pub fn fee(&self, policy_asset: &AssetId) -> u64 {
        self.inner.fee_in((*policy_asset).into())
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
    /// Creates a `Txid` from its hex string representation (64 characters).
    #[wasm_bindgen(constructor)]
    pub fn new(tx_id: &str) -> Result<Txid, Error> {
        Ok(elements::Txid::from_str(tx_id)?.into())
    }

    /// Return the string representation of the transaction identifier as shown in the explorer.
    /// This representation can be used to recreate the transaction identifier via `new()`
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::{AssetId, Transaction, Txid};
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_tx_id() {
        let expected = "HexToArray(InvalidLength(InvalidLengthError { expected: 64, invalid: 2 }))";
        let hex = "xx";
        assert_eq!(expected, format!("{:?}", Txid::new(hex).unwrap_err()));

        let expected = "HexToArray(InvalidChar(InvalidCharError { invalid: 120 }))";
        let hex = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
        assert_eq!(expected, format!("{:?}", Txid::new(hex).unwrap_err()));

        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        assert_eq!(hex, Txid::new(hex).unwrap().to_string());
    }

    #[wasm_bindgen_test]
    async fn test_transaction() {
        let expected = "HexToBytes(InvalidChar(InvalidCharError { invalid: 120 }))";
        let hex = "xx";
        assert_eq!(
            expected,
            format!("{:?}", Transaction::new(hex).unwrap_err())
        );

        let expected =
            include_str!("../../../lwk_jade/test_data/pset_to_be_signed_transaction.hex")
                .to_string();
        let tx = Transaction::new(&expected).unwrap();
        assert_eq!(expected, tx.to_string());

        let expected = "954f32449d00a9de3c42758dedee895c88ea417cb72999738b2631bcc00e13ad";
        assert_eq!(expected, tx.txid().to_string());

        let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        assert_eq!(tx.fee(&AssetId::new(policy_asset).unwrap()), 250);
    }
}
