use lwk_wollet::NoPersist;

use crate::desc::WolletDescriptor;
use crate::network::Network;
use crate::types::{AssetId, SecretKey};
use crate::{
    AddressResult, ExternalUtxo, ForeignPersisterLink, LwkError, Pset, PsetDetails, Transaction,
    Txid, Update, WalletTx, WalletTxOut,
};
use std::sync::{MutexGuard, PoisonError};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// A Watch-Only wallet, wrapper over [`lwk_wollet::Wollet`]
#[derive(uniffi::Object)]
pub struct Wollet {
    inner: Mutex<lwk_wollet::Wollet>, // every exposed method must take `&self` (no &mut) so that we need to encapsulate into Mutex
}
impl Wollet {
    pub(crate) fn inner_wollet(
        &self,
    ) -> Result<MutexGuard<'_, lwk_wollet::Wollet>, PoisonError<MutexGuard<'_, lwk_wollet::Wollet>>>
    {
        self.inner.lock()
    }
}

#[uniffi::export]
impl Wollet {
    /// Construct a Watch-Only wallet object with a caller provided persister
    #[uniffi::constructor]
    pub fn with_custom_persister(
        network: &Network,
        descriptor: &WolletDescriptor,
        persister: Arc<ForeignPersisterLink>,
    ) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_wollet::Wollet::new(network.into(), persister, descriptor.into())?;

        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
        }))
    }

    /// Construct a Watch-Only wallet object
    #[uniffi::constructor]
    pub fn new(
        network: &Network,
        descriptor: &WolletDescriptor,
        datadir: Option<String>,
    ) -> Result<Arc<Self>, LwkError> {
        let inner = match datadir {
            Some(path) => {
                lwk_wollet::Wollet::with_fs_persist(network.into(), descriptor.into(), path)?
            }
            None => lwk_wollet::Wollet::new(network.into(), NoPersist::new(), descriptor.into())?,
        };

        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
        }))
    }

    /// Get a copy of the wallet descriptor
    pub fn descriptor(&self) -> Result<Arc<WolletDescriptor>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.wollet_descriptor().into()))
    }

    /// Get a wallet address
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn address(&self, index: Option<u32>) -> Result<Arc<AddressResult>, LwkError> {
        // TODO test this method assert the first address with many different supported descriptor in different networks
        let wollet = self.inner.lock()?;
        let address = wollet.address(index)?;
        Ok(Arc::new(address.into()))
    }

    /// Return the [ELIP152](https://github.com/ElementsProject/ELIPs/blob/main/elip-0152.mediawiki) deterministic wallet identifier.
    pub fn dwid(&self) -> Result<String, LwkError> {
        let wollet = self.inner.lock()?;
        Ok(wollet.wollet_descriptor().dwid(wollet.network().into())?)
    }

    /// Apply an update containing blockchain data
    ///
    /// To update the wallet you need to first obtain the blockchain data relevant for the wallet.
    /// This can be done using `full_scan()`, which
    /// returns an `Update` that contains new transaction and other data relevant for the
    /// wallet.
    /// The update must then be applied to the `Wollet` so that wollet methods such as
    /// `balance()` or `transactions()` include the new data.
    ///
    /// However getting blockchain data involves network calls, so between the full scan start and
    /// when the update is applied it might elapse a significant amount of time.
    /// In that interval, applying any update, or any transaction using `apply_transaction()`,
    /// will cause this function to return a `Error::UpdateOnDifferentStatus`.
    /// Callers should either avoid applying updates and transactions, or they can catch the error and wait for a new full scan to be completed and applied.
    pub fn apply_update(&self, update: &Update) -> Result<(), LwkError> {
        let mut wollet = self.inner.lock()?;
        wollet.apply_update(update.clone().into())?;
        Ok(())
    }

    /// Apply a transaction to the wallet state
    ///
    /// Wallet transactions are normally obtained using `full_scan()`
    /// and applying the resulting `Update` with `apply_update()`. However a
    /// full scan involves network calls and it can take a significant amount of time.
    ///
    /// If the caller does not want to wait for a full scan containing the transaction, it can
    /// apply the transaction to the wallet state using this function.
    ///
    /// Note: if this transaction is *not* returned by a next full scan, after `apply_update()` it will disappear from the
    /// transactions list, will not be included in balance computations, and by the remaining
    /// wollet methods.
    ///
    /// Calling this method, might cause `apply_update()` to fail with a
    /// `Error::UpdateOnDifferentStatus`, make sure to either avoid it or handle the error properly.
    pub fn apply_transaction(&self, tx: &Transaction) -> Result<(), LwkError> {
        let mut wollet = self.inner.lock()?;
        wollet.apply_transaction(tx.clone().into())?;
        Ok(())
    }

    /// Get the wallet balance
    pub fn balance(&self) -> Result<HashMap<AssetId, u64>, LwkError> {
        let m: HashMap<_, _> = self
            .inner
            .lock()?
            .balance()?
            .as_ref()
            .clone()
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect();
        Ok(m)
    }

    /// Get all the wallet transactions
    pub fn transactions(&self) -> Result<Vec<Arc<WalletTx>>, LwkError> {
        self.transactions_paginated(0, u32::MAX)
    }

    /// Get the wallet transactions with pagination
    pub fn transactions_paginated(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Arc<WalletTx>>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .transactions_paginated(offset as usize, limit as usize)?
            .into_iter()
            .map(Into::into)
            .map(Arc::new)
            .collect())
    }

    /// Get the unspent transaction outputs of the wallet
    pub fn utxos(&self) -> Result<Vec<Arc<WalletTxOut>>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .utxos()?
            .into_iter()
            .map(Into::into)
            .map(Arc::new)
            .collect())
    }

    /// Get all the transaction outputs of the wallet, both spent and unspent
    pub fn txos(&self) -> Result<Vec<Arc<WalletTxOut>>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .txos()?
            .into_iter()
            .map(Into::into)
            .map(Arc::new)
            .collect())
    }

    /// Finalize a PSET, returning a new PSET with the finalized inputs
    pub fn finalize(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let mut pset = pset.inner();
        let wollet = self.inner.lock()?;
        wollet.finalize(&mut pset)?;
        Ok(Arc::new(pset.into()))
    }

    /// Get the PSET details with respect to the wallet
    pub fn pset_details(&self, pset: &Pset) -> Result<Arc<PsetDetails>, LwkError> {
        let wollet = self.inner.lock()?;
        let details = wollet.get_details(&pset.inner())?;
        Ok(Arc::new(details.into()))
    }

    /// Add wallet details to the PSET
    pub fn add_details(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let mut pset = pset.inner();
        let wollet = self.inner.lock()?;
        wollet.add_details(&mut pset)?;
        Ok(Arc::new(pset.into()))
    }

    /// Whether the wallet is segwit
    pub fn is_segwit(&self) -> Result<bool, LwkError> {
        Ok(self.inner.lock()?.is_segwit())
    }

    /// Whether the wallet is AMP0
    pub fn is_amp0(&self) -> Result<bool, LwkError> {
        Ok(self.inner.lock()?.is_amp0())
    }

    /// Max weight to satisfy for inputs belonging to this wallet
    pub fn max_weight_to_satisfy(&self) -> Result<u32, LwkError> {
        Ok(self.inner.lock()?.max_weight_to_satisfy() as u32)
    }

    /// Note this a test method but we are not feature gating in test because we need it in
    /// destination language examples
    pub fn wait_for_tx(
        &self,
        txid: &Txid,
        client: &crate::ElectrumClient,
    ) -> Result<Arc<WalletTx>, LwkError> {
        for _ in 0..30 {
            let update = client.full_scan(self)?;
            if let Some(update) = update {
                self.apply_update(&update)?;
            }
            let mut txs = self.transactions()?;
            txs.retain(|t| *t.txid() == *txid);
            if let Some(tx) = txs.pop() {
                return Ok(tx);
            }

            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        panic!("I wait 30s but I didn't see {txid}");
    }

    /// Get the utxo with unspent transaction outputs of the wallet
    /// Return utxos unblinded with a specific blinding key
    pub fn unblind_utxos_with(
        &self,
        blinding_privkey: &SecretKey,
    ) -> Result<Vec<Arc<ExternalUtxo>>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .unblind_utxos_with(blinding_privkey.into())?
            .into_iter()
            .map(Into::into)
            .map(Arc::new)
            .collect())
    }

    /// Extract the wallet UTXOs that a PSET is creating
    pub fn extract_wallet_utxos(&self, pset: &Pset) -> Result<Vec<Arc<ExternalUtxo>>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .extract_wallet_utxos(&pset.inner())?
            .into_iter()
            .map(Into::into)
            .map(Arc::new)
            .collect())
    }
}
