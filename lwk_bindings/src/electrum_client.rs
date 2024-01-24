use std::sync::{Arc, Mutex};

use wollet::BlockchainBackend;

use crate::{LwkError, Transaction, Txid, Update, Wollet};

#[derive(uniffi::Object, Debug)]
pub struct ElectrumClient {
    inner: Mutex<wollet::ElectrumClient>,
}

#[uniffi::export]
impl ElectrumClient {
    /// Construct a Script object
    #[uniffi::constructor]
    pub fn new(
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
    ) -> Result<Arc<Self>, LwkError> {
        let url = wollet::ElectrumUrl::new(electrum_url, tls, validate_domain);
        let client = wollet::ElectrumClient::new(&url)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
        }))
    }

    pub fn broadcast(&self, tx: &Transaction) -> Result<Arc<Txid>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.broadcast(tx.as_ref())?.into()))
    }

    pub fn full_scan(&self, wollet: &Wollet) -> Result<Option<Arc<Update>>, LwkError> {
        let wollet = wollet.inner_wollet()?;
        let update: Option<wollet::Update> = self.inner.lock()?.full_scan(&wollet)?;
        Ok(update.map(Into::into).map(Arc::new))
    }
}
