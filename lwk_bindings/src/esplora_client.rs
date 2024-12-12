use std::sync::{Arc, Mutex};

use lwk_wollet::clients::blocking::{self, BlockchainBackend};

use crate::{LwkError, Network, Transaction, Txid, Update, Wollet};

/// Wrapper over [`blocking::EsploraClient`]
#[derive(uniffi::Object, Debug)]
pub struct EsploraClient {
    inner: Mutex<blocking::EsploraClient>,
}

#[uniffi::export]
impl EsploraClient {
    /// Construct an Esplora Client
    #[uniffi::constructor]
    pub fn new(url: &str, network: &Network) -> Result<Arc<Self>, LwkError> {
        let client = blocking::EsploraClient::new(url, network.into())?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
        }))
    }

    /// Construct an Esplora Client using Waterfalls endpoint
    #[uniffi::constructor]
    pub fn new_waterfalls(url: &str, network: &Network) -> Result<Arc<Self>, LwkError> {
        let client = blocking::EsploraClient::new_waterfalls(url, network.into())?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
        }))
    }

    pub fn broadcast(&self, tx: &Transaction) -> Result<Arc<Txid>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.broadcast(tx.as_ref())?.into()))
    }

    pub fn full_scan(&self, wollet: &Wollet) -> Result<Option<Arc<Update>>, LwkError> {
        let wollet = wollet.inner_wollet()?;
        let update: Option<lwk_wollet::Update> = self.inner.lock()?.full_scan(&wollet.state())?;
        Ok(update.map(Into::into).map(Arc::new))
    }
}
