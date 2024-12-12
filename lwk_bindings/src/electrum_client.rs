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
        let wollet = wollet.inner_wollet()?;
        let update: Option<lwk_wollet::Update> = self.inner.lock()?.full_scan(&wollet.state())?;
        Ok(update.map(Into::into).map(Arc::new))
    }
}
