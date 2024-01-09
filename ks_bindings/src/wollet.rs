use crate::desc::WolletDescriptor;
use crate::network::ElementsNetwork;
use crate::types::AssetId;
use crate::{AddressResult, Error, Pset, Txid, WalletTx};
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
        network: ElementsNetwork,
        descriptor: Arc<WolletDescriptor>,
        datadir: String,
    ) -> Result<Arc<Self>, Error> {
        let url = network.electrum_url().to_string();
        let inner = wollet::Wollet::new(
            network.into(),
            &url,
            true,
            true,
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
        out_address: String,
        satoshis: u64,
        fee_rate: f32,
    ) -> Result<Arc<Pset>, Error> {
        let wollet = self.inner.lock()?;
        let pset = wollet.send_lbtc(satoshis, &out_address, Some(fee_rate))?;
        Ok(Arc::new(pset.into()))
    }

    pub fn broadcast(&self, pset: Arc<Pset>) -> Result<Arc<Txid>, Error> {
        let mut pset = pset.inner();
        let wollet = self.inner.lock()?;
        wollet.finalize(&mut pset)?;
        let tx = pset.extract_tx()?;
        let txid = wollet.broadcast(&tx)?;
        Ok(Arc::new(txid.into()))
    }
}
