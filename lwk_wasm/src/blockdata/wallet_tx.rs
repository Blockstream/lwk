use crate::{Error, OptionWalletTxOut, Transaction, Txid};
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

/// Wrapper of [`lwk_wollet::WalletTx`]
#[derive(Debug)]
#[wasm_bindgen]
pub struct WalletTx {
    inner: lwk_wollet::WalletTx,
}

impl From<lwk_wollet::WalletTx> for WalletTx {
    fn from(inner: lwk_wollet::WalletTx) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl WalletTx {
    pub fn tx(&self) -> Transaction {
        self.inner.tx.clone().into()
    }

    pub fn height(&self) -> Option<u32> {
        self.inner.height
    }

    pub fn balance(&self) -> Result<JsValue, Error> {
        let serializer = Serializer::new().serialize_large_number_types_as_bigints(true);
        Ok(self.inner.balance.serialize(&serializer)?)
    }

    pub fn txid(&self) -> Txid {
        self.inner.txid.into()
    }

    pub fn fee(&self) -> u64 {
        self.inner.fee
    }

    #[wasm_bindgen(js_name = txType)]
    pub fn tx_type(&self) -> String {
        self.inner.type_.clone()
    }

    pub fn timestamp(&self) -> Option<u32> {
        self.inner.timestamp
    }

    pub fn inputs(&self) -> Vec<OptionWalletTxOut> {
        self.inner
            .inputs
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    pub fn outputs(&self) -> Vec<OptionWalletTxOut> {
        self.inner
            .outputs
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    #[wasm_bindgen(js_name = unblindedUrl)]
    pub fn unblinded_url(&self, explorer_url: &str) -> String {
        self.inner.unblinded_url(explorer_url)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::WalletTx;
    use lwk_wollet::elements::{self, hex::FromHex, pset::serialize::Deserialize};
    use std::collections::HashMap;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn wallet_tx() {
        let tx_out = lwk_wollet::WalletTxOut {
            outpoint: elements::OutPoint::null(),
            script_pubkey: elements::Script::new(),
            height: Some(1),
            unblinded: elements::TxOutSecrets::new(
                elements::AssetId::default(),
                elements::confidential::AssetBlindingFactor::zero(),
                1000,
                elements::confidential::ValueBlindingFactor::zero(),
            ),
            wildcard_index: 10,
            ext_int: lwk_wollet::Chain::External,
        };

        let tx_hex = include_str!("../../../lwk_jade/test_data/pset_to_be_signed_transaction.hex")
            .to_string();
        let tx_bytes = Vec::<u8>::from_hex(&tx_hex).unwrap();
        let tx: elements::Transaction = elements::Transaction::deserialize(&tx_bytes).unwrap();

        let a = elements::AssetId::default();
        let el = lwk_wollet::WalletTx {
            txid: tx.txid(),
            tx: tx.clone(),
            height: Some(4),
            balance: vec![(a, 10)].into_iter().collect(),
            fee: 23,
            type_: "type".to_string(),
            timestamp: Some(124),
            inputs: vec![Some(tx_out.clone())],
            outputs: vec![None, Some(tx_out.clone())],
        };

        let wallet_tx: WalletTx = el.clone().into();

        assert_eq!(wallet_tx.tx(), tx.into());

        assert_eq!(wallet_tx.height(), Some(4));

        let balance: HashMap<elements::AssetId, i64> =
            serde_wasm_bindgen::from_value(wallet_tx.balance().unwrap()).unwrap();
        assert_eq!(balance.get(&a), Some(&10));

        assert_eq!(wallet_tx.fee(), 23);

        assert_eq!(wallet_tx.tx_type(), "type");

        assert_eq!(wallet_tx.timestamp(), Some(124));

        assert_eq!(wallet_tx.inputs().len(), 1);

        assert_eq!(wallet_tx.outputs().len(), 2);
    }
}
