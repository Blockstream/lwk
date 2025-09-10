use crate::{types::AssetId, Transaction, Txid, WalletTxOut};
use std::{collections::HashMap, sync::Arc};

#[derive(uniffi::Object, Debug)]
pub struct WalletTx {
    inner: lwk_wollet::WalletTx,
}

impl From<lwk_wollet::WalletTx> for WalletTx {
    fn from(inner: lwk_wollet::WalletTx) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl WalletTx {
    pub fn tx(&self) -> Arc<Transaction> {
        let tx: Transaction = self.inner.tx.clone().into();
        Arc::new(tx)
    }

    pub fn height(&self) -> Option<u32> {
        self.inner.height
    }

    pub fn balance(&self) -> HashMap<AssetId, i64> {
        self.inner
            .balance
            .iter()
            .map(|(k, v)| (AssetId::from(*k), *v))
            .collect()
    }

    pub fn txid(&self) -> Arc<Txid> {
        Arc::new(self.inner.txid.into())
    }

    pub fn fee(&self) -> u64 {
        self.inner.fee
    }

    pub fn type_(&self) -> String {
        self.inner.type_.clone()
    }

    pub fn timestamp(&self) -> Option<u32> {
        self.inner.timestamp
    }

    pub fn inputs(&self) -> Vec<Option<Arc<WalletTxOut>>> {
        self.inner
            .inputs
            .iter()
            .map(|e| e.as_ref().cloned().map(Into::into).map(Arc::new))
            .collect()
    }

    pub fn outputs(&self) -> Vec<Option<Arc<WalletTxOut>>> {
        self.inner
            .outputs
            .iter()
            .map(|e| e.as_ref().cloned().map(Into::into).map(Arc::new))
            .collect()
    }

    pub fn unblinded_url(&self, explorer_url: &str) -> String {
        self.inner.unblinded_url(explorer_url)
    }
}

#[cfg(test)]
mod tests {
    use crate::WalletTx;
    use elements::{hex::FromHex, pset::serialize::Deserialize, Address};
    use lwk_common::SignedBalance;
    use std::{collections::HashMap, str::FromStr};

    #[test]
    fn wallet_tx() {
        let address_str = "tlq1qqw8re6enadhd82hk9m445kr78e7rlddcu58vypmk9mqa7e989ph30xe8ag7mcqn9rsyu433dcvpas0737sk3sjaqw3484yccj";
        let address = Address::from_str(address_str).unwrap();

        let tx_out = lwk_wollet::WalletTxOut {
            is_spent: false,
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
            address,
        };

        let tx_hex = include_str!("../../../lwk_jade/test_data/pset_to_be_signed_transaction.hex")
            .to_string();
        let tx_bytes = Vec::<u8>::from_hex(&tx_hex).unwrap();
        let tx: elements::Transaction = elements::Transaction::deserialize(&tx_bytes).unwrap();

        let el = lwk_wollet::WalletTx {
            tx: tx.clone(),
            txid: tx.txid(),
            height: Some(4),
            balance: SignedBalance::default(),
            fee: 23,
            type_: "type".to_string(),
            timestamp: Some(124),
            inputs: vec![Some(tx_out.clone())],
            outputs: vec![None, Some(tx_out.clone())],
        };

        let wallet_tx: WalletTx = el.clone().into();

        assert_eq!(*wallet_tx.tx(), tx.into());

        assert_eq!(wallet_tx.height(), Some(4));

        assert_eq!(wallet_tx.balance(), HashMap::new());

        assert_eq!(wallet_tx.fee(), 23);

        assert_eq!(wallet_tx.type_(), "type");

        assert_eq!(wallet_tx.timestamp(), Some(124));

        assert_eq!(wallet_tx.inputs().len(), 1);

        assert_eq!(wallet_tx.outputs().len(), 2);
    }
}
