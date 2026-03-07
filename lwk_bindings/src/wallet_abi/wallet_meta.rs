use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;

use lwk_simplicity::error::WalletAbiError;
use lwk_simplicity::wallet_abi::schema::WalletMeta;
use lwk_wollet::elements::{
    OutPoint as ElementsOutPoint, Transaction as ElementsTransaction, TxOut as ElementsTxOut,
    Txid as ElementsTxid,
};
use lwk_wollet::WalletTxOut as WolletTxOut;

use crate::{LwkError, OutPoint, Transaction, TxOut, Txid, WalletTxOut};

/// Callback interface used by foreign code to provide runtime wallet/backend capabilities.
#[uniffi::export(with_foreign)]
pub trait WalletAbiWalletCallbacks: Send + Sync {
    /// Fetch previous output by outpoint.
    fn get_tx_out(&self, outpoint: Arc<OutPoint>) -> Result<Arc<TxOut>, LwkError>;

    /// Broadcast finalized transaction and return backend-reported txid.
    fn broadcast_transaction(&self, tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError>;

    /// Return spendable wallet UTXOs for runtime input selection.
    fn get_spendable_utxos(&self) -> Result<Vec<Arc<WalletTxOut>>, LwkError>;
}

/// Bridge object adapting foreign wallet callbacks to runtime `WalletMeta`.
#[derive(uniffi::Object)]
pub struct WalletMetaLink {
    pub(crate) inner: Arc<dyn WalletAbiWalletCallbacks>,
}

#[uniffi::export]
impl WalletMetaLink {
    /// Create a wallet bridge from foreign callback implementation.
    #[uniffi::constructor]
    pub fn new(callbacks: Arc<dyn WalletAbiWalletCallbacks>) -> Self {
        Self { inner: callbacks }
    }
}

fn wallet_callback_error(context: &str, error: LwkError) -> WalletAbiError {
    WalletAbiError::InvalidResponse(format!(
        "wallet-abi wallet callback '{context}' failed: {error:?}"
    ))
}

impl WalletMeta for WalletMetaLink {
    type Error = WalletAbiError;

    fn get_tx_out(
        &self,
        outpoint: ElementsOutPoint,
    ) -> impl Future<Output = Result<ElementsTxOut, Self::Error>> + Send + '_ {
        async move {
            let outpoint_bindings = Arc::new(outpoint.into());
            let tx_out = self
                .inner
                .get_tx_out(outpoint_bindings)
                .map_err(|error| wallet_callback_error("get_tx_out", error))?;
            Ok(tx_out.as_ref().into())
        }
    }

    fn broadcast_transaction(
        &self,
        tx: ElementsTransaction,
    ) -> impl Future<Output = Result<ElementsTxid, Self::Error>> + Send + '_ {
        async move {
            let tx_bindings = Arc::new(tx.into());
            let txid = self
                .inner
                .broadcast_transaction(tx_bindings)
                .map_err(|error| wallet_callback_error("broadcast_transaction", error))?;
            Ok(txid.as_ref().into())
        }
    }

    fn get_spendable_utxos(
        &self,
    ) -> impl Future<Output = Result<Vec<WolletTxOut>, Self::Error>> + Send + '_ {
        async move {
            let spendable_utxos = self
                .inner
                .get_spendable_utxos()
                .map_err(|error| wallet_callback_error("get_spendable_utxos", error))?;
            let spendable_utxos = spendable_utxos
                .iter()
                .map(|item| item.as_ref().into())
                .collect::<Vec<WolletTxOut>>();
            validate_spendable_utxos(&spendable_utxos)?;
            Ok(spendable_utxos)
        }
    }
}

fn validate_spendable_utxos(spendable_utxos: &[WolletTxOut]) -> Result<(), WalletAbiError> {
    let mut seen_outpoints = HashSet::new();
    for (index, utxo) in spendable_utxos.iter().enumerate() {
        if utxo.is_spent {
            return Err(WalletAbiError::InvalidResponse(format!(
                "wallet-abi wallet callback 'get_spendable_utxos' returned spent UTXO at index {index}: {}:{}",
                utxo.outpoint.txid, utxo.outpoint.vout
            )));
        }

        if !seen_outpoints.insert(utxo.outpoint) {
            return Err(WalletAbiError::InvalidResponse(format!(
                "wallet-abi wallet callback 'get_spendable_utxos' returned duplicate outpoint at index {index}: {}:{}",
                utxo.outpoint.txid, utxo.outpoint.vout
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;

    use lwk_wollet::elements::{self, OutPoint, TxOutSecrets};
    use lwk_wollet::WalletTxOut as WolletTxOut;

    use super::{WalletAbiWalletCallbacks, WalletMetaLink};
    use crate::{LwkError, Transaction, TxOut, Txid, WalletTxOut};

    fn make_wallet_tx_out(outpoint: elements::OutPoint, is_spent: bool) -> Arc<WalletTxOut> {
        let asset: crate::types::AssetId = crate::UniffiCustomTypeConverter::into_custom(
            "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225".to_string(),
        )
        .expect("static asset");
        let address = elements::Address::from_str("tex1q6rz28mcfaxtmd6v789l9rrlrusdprr9p634wu8")
            .expect("address");
        let wallet_tx_out = WolletTxOut {
            is_spent,
            outpoint,
            script_pubkey: address.script_pubkey(),
            height: Some(1),
            unblinded: TxOutSecrets::new(
                asset.into(),
                elements::confidential::AssetBlindingFactor::zero(),
                50_000,
                elements::confidential::ValueBlindingFactor::zero(),
            ),
            wildcard_index: 0,
            ext_int: lwk_wollet::Chain::External,
            address,
        };
        Arc::new(wallet_tx_out.into())
    }

    #[test]
    fn callback_wallet_snapshot_shape_roundtrip() {
        let asset: crate::types::AssetId = crate::UniffiCustomTypeConverter::into_custom(
            "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225".to_string(),
        )
        .expect("asset");
        let txid = elements::Txid::from_str(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .expect("txid");
        let outpoint = elements::OutPoint::new(txid, 0);
        let address = elements::Address::from_str("tex1q6rz28mcfaxtmd6v789l9rrlrusdprr9p634wu8")
            .expect("address");
        let txo = WolletTxOut {
            is_spent: false,
            outpoint,
            script_pubkey: address.script_pubkey(),
            height: Some(1),
            unblinded: TxOutSecrets::new(
                asset.into(),
                elements::confidential::AssetBlindingFactor::zero(),
                1234,
                elements::confidential::ValueBlindingFactor::zero(),
            ),
            wildcard_index: 0,
            ext_int: lwk_wollet::Chain::External,
            address,
        };
        let wrapped: WalletTxOut = txo.clone().into();
        let unwrapped: WolletTxOut = (&wrapped).into();
        assert_eq!(unwrapped.unblinded.value, txo.unblinded.value);
    }

    struct TestWalletCallbacks {
        tx_out: Arc<TxOut>,
        txid: Arc<Txid>,
        spendable_utxos: Vec<Arc<WalletTxOut>>,
    }

    impl TestWalletCallbacks {
        fn new() -> Self {
            let script = crate::Script::empty();
            let asset: crate::types::AssetId = crate::UniffiCustomTypeConverter::into_custom(
                "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225".to_string(),
            )
            .expect("static asset");
            let tx_out = TxOut::from_explicit(&script, asset, 50_000);
            let txid = Txid::new(
                &"0000000000000000000000000000000000000000000000000000000000000001"
                    .parse()
                    .expect("static txid"),
            )
            .expect("static txid");

            let outpoint = elements::OutPoint::new(
                elements::Txid::from_str(&txid.to_string()).expect("txid"),
                0,
            );
            Self {
                tx_out,
                txid,
                spendable_utxos: vec![make_wallet_tx_out(outpoint, false)],
            }
        }
    }

    impl WalletAbiWalletCallbacks for TestWalletCallbacks {
        fn get_tx_out(&self, _outpoint: Arc<crate::OutPoint>) -> Result<Arc<TxOut>, LwkError> {
            Ok(self.tx_out.clone())
        }

        fn broadcast_transaction(&self, _tx: Arc<Transaction>) -> Result<Arc<Txid>, LwkError> {
            Ok(self.txid.clone())
        }

        fn get_spendable_utxos(&self) -> Result<Vec<Arc<WalletTxOut>>, LwkError> {
            Ok(self.spendable_utxos.clone())
        }
    }

    #[test]
    fn wallet_meta_link_bridges_callbacks() {
        let callbacks = Arc::new(TestWalletCallbacks::new());
        let link = WalletMetaLink::new(callbacks.clone());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");

        let outpoint = OutPoint::new(
            elements::Txid::from_str(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .expect("txid"),
            0,
        );
        let tx_out = runtime
            .block_on(
                <WalletMetaLink as lwk_simplicity::wallet_abi::schema::WalletMeta>::get_tx_out(
                    &link, outpoint,
                ),
            )
            .expect("get_tx_out");
        assert_eq!(tx_out.value.explicit(), Some(50_000));

        let tx = elements::Transaction {
            version: 2,
            lock_time: elements::LockTime::ZERO,
            input: vec![],
            output: vec![],
        };
        let txid = runtime
            .block_on(
                <WalletMetaLink as lwk_simplicity::wallet_abi::schema::WalletMeta>::broadcast_transaction(
                    &link, tx,
                ),
            )
            .expect("broadcast");
        assert_eq!(
            txid.to_string(),
            "0000000000000000000000000000000000000000000000000000000000000001"
        );

        let spendable_utxos = runtime
            .block_on(
                <WalletMetaLink as lwk_simplicity::wallet_abi::schema::WalletMeta>::get_spendable_utxos(
                    &link,
                ),
            )
            .expect("snapshot");
        assert_eq!(spendable_utxos.len(), 1);
        assert_eq!(spendable_utxos[0].unblinded.value, 50_000);
    }
}
