use std::sync::{Arc, Mutex};

use lwk_wollet::clients::blocking::BlockchainBackend;

use crate::{LwkError, Transaction, Txid, Update, Wollet};

/// Wrapper over [`lwk_wollet::ElectrumClient`]
#[derive(uniffi::Object, Debug)]
pub struct ElectrumClient {
    inner: Mutex<lwk_wollet::ElectrumClient>,
}

#[uniffi::export]
impl ElectrumClient {
    /// Construct an Electrum client
    #[uniffi::constructor]
    pub fn new(
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
    ) -> Result<Arc<Self>, LwkError> {
        let url = lwk_wollet::ElectrumUrl::new(electrum_url, tls, validate_domain)
            .map_err(lwk_wollet::Error::Url)?;
        let client = lwk_wollet::ElectrumClient::new(&url)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
        }))
    }

    pub fn ping(&self) -> Result<(), LwkError> {
        Ok(self.inner.lock()?.ping()?)
    }

    pub fn broadcast(&self, tx: &Transaction) -> Result<Arc<Txid>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.broadcast(tx.as_ref())?.into()))
    }

    pub fn full_scan(&self, wollet: &Wollet) -> Result<Option<Arc<Update>>, LwkError> {
        self.full_scan_to_index(wollet, 0)
    }

    pub fn full_scan_to_index(
        &self,
        wollet: &Wollet,
        index: u32,
    ) -> Result<Option<Arc<Update>>, LwkError> {
        let wollet = wollet.inner_wollet()?;
        let update: Option<lwk_wollet::Update> = self
            .inner
            .lock()?
            .full_scan_to_index(&wollet.state(), index)?;
        Ok(update.map(Into::into).map(Arc::new))
    }
}
