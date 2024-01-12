use crate::desc::WolletDescriptor;
use crate::network::ElementsNetwork;
use crate::types::AssetId;
use crate::{Address, AddressResult, ElectrumUrl, Error, Pset, Txid, WalletTx};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// A Watch-Only wallet
#[derive(uniffi::Object)]
pub struct Wollet {
    inner: Mutex<wollet::Wollet>, // every exposed method must take `&self` (no &mut) so that we need to encapsulate into Mutex
}

#[uniffi::export]
impl Wollet {
    /// Construct a Watch-Only wallet object
    #[uniffi::constructor]
    pub fn new(
        network: &ElementsNetwork,
        descriptor: &WolletDescriptor,
        datadir: String,
        electrum_url: &ElectrumUrl,
    ) -> Result<Arc<Self>, Error> {
        let inner = wollet::Wollet::new(
            (*network).into(),
            &electrum_url.url,
            electrum_url.tls,
            electrum_url.validate_domain,
            &datadir,
            &descriptor.to_string(),
        )?;
        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
        }))
    }

    pub fn descriptor(&self) -> Result<Arc<WolletDescriptor>, Error> {
        Ok(Arc::new(self.inner.lock()?.wollet_descriptor().into()))
    }

    pub fn address(&self, index: Option<u32>) -> Result<Arc<AddressResult>, Error> {
        // TODO test this method assert the first address with many different supported descriptor in different networks
        let wollet = self.inner.lock()?;
        let address = wollet.address(index)?;
        Ok(Arc::new(address.into()))
    }

    pub fn sync(&self) -> Result<(), Error> {
        let mut wollet = self.inner.lock()?;
        wollet.sync_tip()?;
        wollet.sync_txs()?;
        Ok(())
    }

    pub fn balance(&self) -> Result<HashMap<AssetId, u64>, Error> {
        let m: HashMap<_, _> = self
            .inner
            .lock()?
            .balance()?
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect();
        Ok(m)
    }

    pub fn transactions(&self) -> Result<Vec<Arc<WalletTx>>, Error> {
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
    ) -> Result<Arc<Pset>, Error> {
        let wollet = self.inner.lock()?;
        let pset = wollet.send_lbtc(satoshis, &out_address.to_string(), Some(fee_rate))?;
        Ok(Arc::new(pset.into()))
    }

    pub fn broadcast(&self, pset: &Pset) -> Result<Arc<Txid>, Error> {
        let mut pset = pset.inner();
        let wollet = self.inner.lock()?;
        wollet.finalize(&mut pset)?;
        let tx = pset.extract_tx()?;
        let txid = wollet.broadcast(&tx)?;
        Ok(Arc::new(txid.into()))
    }
}

#[cfg(test)]
impl Wollet {
    pub fn wait_for_tx(&self, txid: Txid) {
        for _ in 0..30 {
            self.sync().unwrap();
            let txs = self.transactions().unwrap();
            if txs.iter().any(|t| *t.txid() == txid) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        panic!("I wait 30s but I didn't see {}", txid);
    }
}
