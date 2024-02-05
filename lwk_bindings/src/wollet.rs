use lwk_wollet::NoPersist;

use crate::desc::WolletDescriptor;
use crate::network::Network;
use crate::types::AssetId;
use crate::{Address, AddressResult, ForeignPersisterLink, LwkError, Pset, Update, WalletTx};
use std::sync::{MutexGuard, PoisonError};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// A Watch-Only wallet
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
        let inner = lwk_wollet::Wollet::new((*network).into(), persister, descriptor.into())?;

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
                lwk_wollet::Wollet::with_fs_persist((*network).into(), descriptor.into(), path)?
            }
            None => {
                lwk_wollet::Wollet::new((*network).into(), NoPersist::new(), descriptor.into())?
            }
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
        Ok(self
            .inner
            .lock()?
            .transactions()?
            .into_iter()
            .map(Into::into)
            .map(Arc::new)
            .collect())
    }

    pub fn create_lbtc_tx(
        &self,
        out_address: &Address,
        satoshis: u64,
        fee_rate: f32,
    ) -> Result<Arc<Pset>, LwkError> {
        let wollet = self.inner.lock()?;
        let pset = wollet.send_lbtc(satoshis, &out_address.to_string(), Some(fee_rate))?;
        Ok(Arc::new(pset.into()))
    }

    pub fn finalize(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let mut pset = pset.inner();
        let wollet = self.inner.lock()?;
        wollet.finalize(&mut pset)?;
        Ok(Arc::new(pset.into()))
    }
}

#[cfg(test)]
impl Wollet {
    pub fn wait_for_tx(&self, txid: crate::Txid, client: &crate::ElectrumClient) {
        for _ in 0..30 {
            let update = client.full_scan(self).unwrap();
            if let Some(update) = update {
                self.apply_update(&update).unwrap();
                let txs = self.transactions().unwrap();
                if txs.iter().any(|t| *t.txid() == txid) {
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        panic!("I wait 30s but I didn't see {}", txid);
    }
}
