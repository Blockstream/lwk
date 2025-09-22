use crate::{Balance, OptionWalletTxOut, Transaction, Txid};
use wasm_bindgen::prelude::*;

/// Value returned by asking transactions to the wallet. Contains details about a transaction
/// from the perspective of the wallet, for example the net-balance of the transaction for the
/// wallet.
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
    /// Return a copy of the transaction.
    pub fn tx(&self) -> Transaction {
        self.inner.tx.clone().into()
    }

    /// Return the height of the block containing the transaction if it's confirmed.
    pub fn height(&self) -> Option<u32> {
        self.inner.height
    }

    /// Return the net balance of the transaction for the wallet.
    pub fn balance(&self) -> Balance {
        self.inner.balance.clone().into()
    }

    /// Return the transaction identifier.
    pub fn txid(&self) -> Txid {
        self.inner.txid.into()
    }

    /// Return the fee of the transaction.
    pub fn fee(&self) -> u64 {
        self.inner.fee
    }

    /// Return the type of the transaction. Can be "issuance", "reissuance", "burn", "redeposit", "incoming", "outgoing" or "unknown".
    #[wasm_bindgen(js_name = txType)]
    pub fn tx_type(&self) -> String {
        self.inner.type_.clone()
    }

    /// Return the timestamp of the block containing the transaction if it's confirmed.
    pub fn timestamp(&self) -> Option<u32> {
        self.inner.timestamp
    }

    /// Return a list with the same number of elements as the inputs of the transaction.
    /// The element in the list is a `WalletTxOut` (the output spent to create the input)
    /// if it belongs to the wallet, while it is None for inputs owned by others
    pub fn inputs(&self) -> Vec<OptionWalletTxOut> {
        self.inner
            .inputs
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Return a list with the same number of elements as the outputs of the transaction.
    /// The element in the list is a `WalletTxOut` if it belongs to the wallet,
    /// while it is None for inputs owned by others
    pub fn outputs(&self) -> Vec<OptionWalletTxOut> {
        self.inner
            .outputs
            .clone()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Return the URL to the transaction on the given explorer including the information
    /// needed to unblind the transaction in the explorer UI.
    #[wasm_bindgen(js_name = unblindedUrl)]
    pub fn unblinded_url(&self, explorer_url: &str) -> String {
        self.inner.unblinded_url(explorer_url)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::WalletTx;
    use lwk_wollet::elements::{self, hex::FromHex, pset::serialize::Deserialize};
    use std::collections::{BTreeMap, HashMap};
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn wallet_tx() {
        let address_str = "tlq1qqw8re6enadhd82hk9m445kr78e7rlddcu58vypmk9mqa7e989ph30xe8ag7mcqn9rsyu433dcvpas0737sk3sjaqw3484yccj";

        let address = crate::Address::new(address_str).unwrap();

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
            is_spent: false,
            address: address.into(),
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
            balance: vec![(a, 10)].into_iter().collect::<BTreeMap<_, _>>().into(),
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
            serde_wasm_bindgen::from_value(wallet_tx.balance().entries().unwrap()).unwrap();
        assert_eq!(balance.get(&a), Some(&10));

        assert_eq!(wallet_tx.fee(), 23);

        assert_eq!(wallet_tx.tx_type(), "type");

        assert_eq!(wallet_tx.timestamp(), Some(124));

        assert_eq!(wallet_tx.inputs().len(), 1);

        assert_eq!(wallet_tx.outputs().len(), 2);
    }
}
