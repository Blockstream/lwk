//! Liquid transaction

use elements::{
    hex::ToHex,
    pset::serialize::{Deserialize, Serialize},
};
use lwk_wollet::{hashes::hex::FromHex, WalletTx, EC};

use crate::{
    types::{AssetId, Hex},
    LwkError, TxIn, TxOut, Txid,
};
use std::{fmt::Display, sync::Arc};

/// A Liquid transaction
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone)]
#[uniffi::export(Display)]
pub struct Transaction {
    inner: elements::Transaction,
}

impl From<WalletTx> for Transaction {
    fn from(value: WalletTx) -> Self {
        Self { inner: value.tx }
    }
}

impl From<elements::Transaction> for Transaction {
    fn from(inner: elements::Transaction) -> Self {
        Self { inner }
    }
}

impl From<Transaction> for elements::Transaction {
    fn from(value: Transaction) -> Self {
        value.inner
    }
}

impl From<&Transaction> for elements::Transaction {
    fn from(value: &Transaction) -> Self {
        value.inner.clone()
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_bytes().to_hex())
    }
}

impl AsRef<elements::Transaction> for Transaction {
    fn as_ref(&self) -> &elements::Transaction {
        &self.inner
    }
}

#[uniffi::export]
impl Transaction {
    /// Construct a Transaction object from its hex representation.
    /// To create the hex representation of a transaction use `to_string()`.
    ///
    /// Deprecated: use `from_string()` instead.
    #[uniffi::constructor]
    pub fn new(hex: &Hex) -> Result<Arc<Self>, LwkError> {
        let inner: elements::Transaction = elements::Transaction::deserialize(hex.as_ref())?;
        Ok(Arc::new(Self { inner }))
    }

    /// Construct a Transaction object from its bytes.
    #[uniffi::constructor]
    pub fn from_bytes(bytes: &[u8]) -> Result<Arc<Self>, LwkError> {
        let inner: elements::Transaction = elements::Transaction::deserialize(bytes)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Construct a Transaction object from its canonical string representation.
    /// To create the string representation of a transaction use `to_string()`.
    #[uniffi::constructor]
    pub fn from_string(s: &str) -> Result<Arc<Self>, LwkError> {
        let bytes = Vec::<u8>::from_hex(s)?;
        let inner: elements::Transaction = elements::Transaction::deserialize(&bytes)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Return the transaction identifier.
    pub fn txid(&self) -> Arc<Txid> {
        Arc::new(self.inner.txid().into())
    }

    /// Return the consensus encoded bytes of the transaction.
    ///
    ///  Deprecated: use `to_bytes()` instead.
    pub fn bytes(&self) -> Vec<u8> {
        elements::Transaction::serialize(&self.inner)
    }

    /// Return the consensus encoded bytes of the transaction.
    pub fn to_bytes(&self) -> Vec<u8> {
        elements::Transaction::serialize(&self.inner)
    }

    /// Returns the "discount virtual size" of this transaction.
    pub fn discount_vsize(&self) -> u32 {
        self.inner.discount_vsize() as u32
    }

    /// Return the fee of the transaction in the given asset.
    /// At the moment the only asset that can be used as fee is the policy asset (LBTC for mainnet).
    pub fn fee(&self, policy_asset: &AssetId) -> u64 {
        self.inner.fee_in((*policy_asset).into())
    }

    /// Return a copy of the outputs of the transaction.
    pub fn outputs(&self) -> Vec<Arc<TxOut>> {
        self.inner
            .output
            .iter()
            .map(|o| Arc::new(o.clone().into()))
            .collect()
    }

    /// Return a copy of the inputs of the transaction.
    pub fn inputs(&self) -> Vec<Arc<TxIn>> {
        self.inner
            .input
            .iter()
            .map(|i| Arc::new(i.clone().into()))
            .collect()
    }

    /// Verify that the transaction has correctly calculated blinding factors and they CT
    /// verification equation holds.
    ///
    /// This is *NOT* a complete Transaction verification check
    /// It does *NOT* check whether input witness/script satisfies the script pubkey, or
    /// inputs are double-spent and other consensus checks.
    ///
    /// This method only checks if the `Transaction` verification equation for Confidential
    /// transactions holds. i.e Sum of inputs = Sum of outputs + fees.
    ///
    /// And the corresponding surjection/rangeproofs are correct.
    /// For checking of surjection proofs and amounts, spent_utxos parameter
    /// should contain information about the prevouts. Note that the order of
    /// spent_utxos should be consistent with transaction inputs.
    pub fn verify_tx_amt_proofs(&self, utxos: Vec<Arc<TxOut>>) -> Result<(), LwkError> {
        let utxos_inner: Vec<elements::TxOut> = utxos.iter().map(|u| u.as_ref().into()).collect();
        self.inner.verify_tx_amt_proofs(&EC, &utxos_inner)?;
        Ok(())
    }
}

/// Editor for modifying transactions.
#[cfg(feature = "simplicity")]
#[derive(uniffi::Object, Debug)]
pub struct TransactionEditor {
    inner: std::sync::Mutex<Option<elements::Transaction>>,
}

#[cfg(feature = "simplicity")]
#[uniffi::export]
impl TransactionEditor {
    /// Create an editor from an existing transaction.
    #[uniffi::constructor]
    pub fn from_transaction(tx: &Transaction) -> Arc<Self> {
        Arc::new(Self {
            inner: std::sync::Mutex::new(Some(tx.inner.clone())),
        })
    }

    /// Set the witness for a specific input.
    pub fn set_input_witness(
        &self,
        input_index: u32,
        witness: &crate::blockdata::tx_in_witness::TxInWitness,
    ) -> Result<(), LwkError> {
        let idx = input_index as usize;
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or(LwkError::ObjectConsumed)?;
        if idx >= inner.input.len() {
            return Err(LwkError::Generic {
                msg: format!(
                    "Input index {} out of bounds (transaction has {} inputs)",
                    input_index,
                    inner.input.len()
                ),
            });
        }
        inner.input[idx].witness = witness.as_ref().clone();
        Ok(())
    }

    /// Build the transaction, consuming the editor.
    pub fn build(&self) -> Result<Arc<Transaction>, LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(Arc::new(Transaction { inner }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction() {
        let tx_expected_hex =
            include_str!("../../../lwk_jade/test_data/pset_to_be_signed_transaction.hex")
                .to_string();
        let tx_expected_bytes = Vec::from_hex(&tx_expected_hex).unwrap();
        let tx_str = Transaction::from_string(&tx_expected_hex).unwrap();
        let tx_bytes = Transaction::from_bytes(&tx_expected_bytes).unwrap();

        assert_eq!(tx_expected_hex, tx_str.to_string());
        assert_eq!(tx_expected_bytes, tx_str.to_bytes());

        assert_eq!(
            tx_str.txid().to_string(),
            "954f32449d00a9de3c42758dedee895c88ea417cb72999738b2631bcc00e13ad"
        );
        assert_eq!(
            tx_bytes.txid().to_string(),
            "954f32449d00a9de3c42758dedee895c88ea417cb72999738b2631bcc00e13ad"
        );
    }

    #[test]
    fn external_unblind() {
        let network = crate::network::Network::regtest_default();
        let desc = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d";
        let desc = crate::WolletDescriptor::new(desc).unwrap();
        let tx_hex = include_str!("../../tests/test_data/tx.hex").to_string();
        let tx = Transaction::from_string(&tx_hex).unwrap();
        for output in tx.outputs() {
            if output.is_fee() {
                assert!(!output.is_partially_blinded());
                assert_eq!(output.asset().unwrap(), network.policy_asset());
                assert_eq!(output.value().unwrap(), 250);
                assert!(output.script_pubkey().bytes().is_empty());
            } else {
                assert!(output.is_partially_blinded());
                assert!(output.asset().is_none());
                assert!(output.value().is_none());
                let script_pubkey = output.script_pubkey();
                assert!(!script_pubkey.bytes().is_empty());
                let private_blinding_key = desc.derive_blinding_key(&script_pubkey).unwrap();
                let txout_secrets = output.unblind(&private_blinding_key).unwrap();
                assert_eq!(txout_secrets.asset(), network.policy_asset());
            }
        }
        tx.outputs().iter().find(|o| o.is_fee()).unwrap();
        tx.outputs().iter().find(|o| !o.is_fee()).unwrap();
    }
}
