use lwk_wollet::NoPersist;

use crate::desc::WolletDescriptor;
use crate::network::Network;
use crate::types::{AssetId, SecretKey};
use crate::{
    AddressResult, ExternalUtxo, ForeignPersisterLink, LwkError, Pset, PsetDetails, Txid, Update,
    WalletTx, WalletTxOut,
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
    pub fn inner_wollet(
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

    pub fn descriptor(&self) -> Result<Arc<WolletDescriptor>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.wollet_descriptor().into()))
    }

    pub fn address(&self, index: Option<u32>) -> Result<Arc<AddressResult>, LwkError> {
        // TODO test this method assert the first address with many different supported descriptor in different networks
        let wollet = self.inner.lock()?;
        let address = wollet.address(index)?;
        Ok(Arc::new(address.into()))
    }

    pub fn apply_update(&self, update: &Update) -> Result<(), LwkError> {
        let mut wollet = self.inner.lock()?;
        wollet.apply_update(update.clone().into())?;
        Ok(())
    }

    pub fn balance(&self) -> Result<HashMap<AssetId, u64>, LwkError> {
        let m: HashMap<_, _> = self
            .inner
            .lock()?
            .balance()?
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect();
        Ok(m)
    }

    pub fn transactions(&self) -> Result<Vec<Arc<WalletTx>>, LwkError> {
        self.transactions_paginated(0, u32::MAX)
    }

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

    pub fn finalize(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let mut pset = pset.inner();
        let wollet = self.inner.lock()?;
        wollet.finalize(&mut pset)?;
        Ok(Arc::new(pset.into()))
    }

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
        panic!("I wait 30s but I didn't see {}", txid);
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
