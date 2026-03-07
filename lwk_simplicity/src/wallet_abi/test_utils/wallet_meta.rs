use std::future::Future;

use lwk_wollet::asyncr::EsploraClient;
use lwk_wollet::elements::{OutPoint, Transaction, TxOut, Txid};
use lwk_wollet::{WalletTxOut, Wollet};
use tokio::sync::Mutex;

use crate::error::WalletAbiError;
use crate::wallet_abi::schema::WalletMeta;

/// Test implementation of [`WalletMeta`] backed by an async Esplora backend and wallet.
pub struct TestWalletMeta {
    backend: Mutex<EsploraClient>,
    wallet: Mutex<Wollet>,
}

impl TestWalletMeta {
    /// Build test wallet metadata from a backend and a wallet instance.
    pub fn new(backend: EsploraClient, wallet: Wollet) -> Self {
        Self {
            backend: Mutex::new(backend),
            wallet: Mutex::new(wallet),
        }
    }

    /// Run backend full scan and apply the update to the wallet if present.
    pub async fn sync_wallet(&self) -> Result<(), WalletAbiError> {
        let mut backend = self.backend.lock().await;
        let mut wallet = self.wallet.lock().await;

        let update = backend.full_scan(&wallet).await.map_err(|error| {
            WalletAbiError::InvalidResponse(format!("wallet full scan failed: {error}"))
        })?;

        if let Some(update) = update {
            wallet.apply_update(update).map_err(|error| {
                WalletAbiError::InvalidResponse(format!("applying wallet update failed: {error}"))
            })?;
        }

        Ok(())
    }

    /// Return previous output for a given outpoint.
    pub async fn get_tx_out(&self, outpoint: &OutPoint) -> Result<TxOut, WalletAbiError> {
        let tx = self.get_transaction(outpoint.txid).await?;

        tx.output
            .get(outpoint.vout as usize)
            .cloned()
            .ok_or_else(|| {
                WalletAbiError::InvalidRequest(format!(
                    "prevout transaction {} missing vout {}",
                    outpoint.txid, outpoint.vout
                ))
            })
    }

    /// Fetch a transaction by txid from backend.
    pub async fn get_transaction(&self, txid: Txid) -> Result<Transaction, WalletAbiError> {
        let backend = self.backend.lock().await;
        backend.get_transaction(txid).await.map_err(|error| {
            WalletAbiError::InvalidResponse(format!("failed to fetch transaction {txid}: {error}"))
        })
    }

    /// Broadcast transaction via backend.
    pub async fn broadcast(&self, tx: &Transaction) -> Result<Txid, WalletAbiError> {
        let backend = self.backend.lock().await;
        backend
            .broadcast(tx)
            .await
            .map_err(|error| WalletAbiError::InvalidResponse(format!("broadcast failed: {error}")))
    }

    /// Return spendable wallet UTXOs used for runtime input selection.
    pub async fn spendable_utxos(&self) -> Result<Vec<WalletTxOut>, WalletAbiError> {
        let wallet = self.wallet.lock().await;
        wallet.utxos().map_err(|error| {
            WalletAbiError::InvalidResponse(format!("reading wallet utxos failed: {error}"))
        })
    }
}

impl WalletMeta for TestWalletMeta {
    type Error = WalletAbiError;

    fn get_tx_out(
        &self,
        outpoint: OutPoint,
    ) -> impl Future<Output = Result<TxOut, Self::Error>> + Send + '_ {
        async move { TestWalletMeta::get_tx_out(self, &outpoint).await }
    }

    fn broadcast_transaction(
        &self,
        tx: Transaction,
    ) -> impl Future<Output = Result<Txid, Self::Error>> + Send + '_ {
        async move { TestWalletMeta::broadcast(self, &tx).await }
    }

    fn get_spendable_utxos(
        &self,
    ) -> impl Future<Output = Result<Vec<WalletTxOut>, Self::Error>> + Send + '_ {
        async move { TestWalletMeta::spendable_utxos(self).await }
    }
}
