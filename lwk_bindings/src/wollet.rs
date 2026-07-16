use crate::desc::WolletDescriptor;
use crate::network::Network;
use crate::store::ForeignStoreLink;
use crate::types::{AssetId, SecretKey};
use crate::{
    AddressResult, ExternalUtxo, LwkError, Pset, PsetDetails, Transaction, Txid, Update, WalletTx,
    WalletTxOut,
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

/// A builder for constructing a [`Wollet`].
#[derive(uniffi::Object)]
pub struct WolletBuilder {
    /// Uniffi doesn't allow to accept self and consume the parameter (everything is behind Arc)
    /// So, inside the Mutex we have an option that allow to consume the inner builder and also
    /// to emulate the consumption of this builder after the call to finish.
    inner: Mutex<Option<lwk_wollet::WolletBuilder>>,
}
impl Wollet {
    pub(crate) fn inner_wollet(
        &self,
    ) -> Result<MutexGuard<'_, lwk_wollet::Wollet>, PoisonError<MutexGuard<'_, lwk_wollet::Wollet>>>
    {
        self.inner.lock()
    }

    fn from_inner(inner: lwk_wollet::Wollet) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(inner),
        })
    }
}

#[uniffi::export]
impl Wollet {
    /// Construct a Watch-Only wallet object with a caller provided store
    #[uniffi::constructor]
    pub fn with_custom_store(
        network: &Network,
        descriptor: &WolletDescriptor,
        store: Arc<ForeignStoreLink>,
    ) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_wollet::Wollet::new(network.into(), store, descriptor.into())?;
        Ok(Self::from_inner(inner))
    }

    /// Construct a Watch-Only wallet object
    #[uniffi::constructor]
    pub fn new(
        network: &Network,
        descriptor: &WolletDescriptor,
        datadir: Option<String>,
    ) -> Result<Arc<Self>, LwkError> {
        let mut builder = lwk_wollet::WolletBuilder::new(network.into(), descriptor.into());
        if let Some(path) = datadir {
            builder = builder.with_legacy_fs_store(&path)?;
        }
        Ok(Self::from_inner(builder.build()?))
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
        Ok(wollet.wollet_descriptor().dwid(wollet.network())?)
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
        let opt = lwk_wollet::TxsOpt {
            offset: Some(offset as usize),
            limit: Some(limit as usize),
            ..Default::default()
        };
        let mut txs: Vec<WalletTx> = self
            .inner
            .lock()?
            .txs(&opt)?
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        txs.retain(WalletTx::is_relevant);
        Ok(txs.into_iter().map(Arc::new).collect())
    }

    /// Get all the wallet transaction
    pub fn transaction(&self, txid: &Txid) -> Result<Arc<WalletTx>, LwkError> {
        let err = || LwkError::Generic {
            msg: "tx not found".to_string(),
        };
        Ok(Arc::new(
            self.inner
                .lock()?
                .transaction(&txid.into())?
                .ok_or_else(err)?
                .into(),
        ))
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

#[uniffi::export]
impl WolletBuilder {
    /// Create a builder for a watch-only wallet.
    #[uniffi::constructor]
    pub fn new(network: &Network, descriptor: &WolletDescriptor) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Some(lwk_wollet::WolletBuilder::new(
                network.into(),
                descriptor.into(),
            ))),
        })
    }

    /// Set the threshold used to merge persisted updates during build.
    ///
    /// **Experimental**: This API may change without notice.
    ///
    /// `None` disables merging (default behavior).
    pub fn with_merge_threshold(&self, merge_threshold: Option<u32>) -> Result<(), LwkError> {
        let mut inner = self.inner.lock()?;
        let builder = inner.take().ok_or(LwkError::ObjectConsumed)?;
        *inner = Some(builder.with_merge_threshold(merge_threshold.map(|t| t as usize)));
        Ok(())
    }

    /// Set the wallet as "utxo only"
    ///
    /// **Experimental**: This API may change without notice.
    pub fn utxo_only(&self, utxo_only: bool) -> Result<(), LwkError> {
        let mut inner = self.inner.lock()?;
        let builder = inner.take().ok_or(LwkError::ObjectConsumed)?;
        *inner = Some(builder.utxo_only(utxo_only));
        Ok(())
    }

    /// Persist wallet updates in the legacy encrypted filesystem store.
    pub fn with_legacy_fs_store(&self, datadir: &str) -> Result<(), LwkError> {
        let mut inner = self.inner.lock()?;
        let builder = inner.take().ok_or(LwkError::ObjectConsumed)?;
        *inner = Some(builder.with_legacy_fs_store(datadir)?);
        Ok(())
    }

    /// Build the wallet from this builder.
    pub fn build(&self) -> Result<Arc<Wollet>, LwkError> {
        let mut inner = self.inner.lock()?;
        let builder = inner.take().ok_or(LwkError::ObjectConsumed)?;
        let wollet = builder.build()?;
        Ok(Wollet::from_inner(wollet))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lwk_wollet::bitcoin::hashes::Hash;

    const REDEPOSIT_DESCRIPTOR: &str = "ct(slip77(a8f5c7be6fbf3eaccf80f907c20e677b3e33223b4f86699991522fbcb0a0381d),elwpkh([9869f387/84'/1'/0']tpubDCKAvXbLyJxHn8GbcgLna71N4tkphUqppBLMss1eumTockyixAHkPqGNZJXzBvsQ3EpnUbGvUd56CMTdcVRDvdaLzYbuP7uBt2R6pMkizy2/<0;1>/*))#4mg5xhpd";
    const WATERFALLS_URL: &str = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";

    #[test]
    fn test_new_with_datadir_loads_encrypted_update_backward_compat() {
        let desc_string = lwk_test_util::wollet_descriptor_string2();
        let desc = WolletDescriptor::new(&desc_string).unwrap();
        let enc_bytes = lwk_test_util::update_test_vector_encrypted_bytes2();

        let network = Network::testnet();
        let datadir = tempfile::tempdir().unwrap();

        let mut update_path = datadir.path().to_path_buf();
        update_path.push(network.inner.as_str());
        update_path.push("enc_cache");
        update_path
            .push(<lwk_wollet::DirectoryIdHash as Hash>::hash(desc_string.as_bytes()).to_string());
        std::fs::create_dir_all(&update_path).unwrap();
        update_path.push("000000000000");
        std::fs::write(update_path, &enc_bytes).unwrap();

        let wollet = Wollet::new(
            network.as_ref(),
            desc.as_ref(),
            Some(datadir.path().to_str().unwrap().to_string()),
        )
        .unwrap();

        let inner = wollet.inner_wollet().unwrap();
        assert_eq!(inner.updates().unwrap().len(), 1);
        assert_eq!(inner.tip().height(), 1360180);
    }

    #[test]
    #[ignore = "requires the production Waterfalls testnet server"]
    fn redeposit_is_classified_correctly() {
        let network = Network::testnet();
        let descriptor = WolletDescriptor::new(REDEPOSIT_DESCRIPTOR).unwrap();
        let wollet = Wollet::new(&network, &descriptor, None).unwrap();
        let client = crate::EsploraClient::new_waterfalls(WATERFALLS_URL, &network).unwrap();

        if let Some(update) = client.full_scan(&wollet).unwrap() {
            wollet.apply_update(&update).unwrap();
        }

        let policy_asset = network.policy_asset();
        let mut found_redeposit = false;
        for tx in wollet.transactions().unwrap() {
            let fee = tx.fee();
            let balance = tx.balance();
            if fee > 0 && balance.len() == 1 && balance.get(&policy_asset) == Some(&-(fee as i64)) {
                assert_eq!(tx.type_(), "redeposit");
                found_redeposit = true;
            }
        }
        assert!(found_redeposit, "the redeposit transaction was not found");
    }
}
