use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};

use crate::{Error, Transaction, Txid, Update, Wollet};

#[derive(uniffi::Object, Debug)]
pub struct ElectrumClient {
    inner: Mutex<wollet::ElectrumClient>,
}

#[uniffi::export]
impl ElectrumClient {
    /// Construct a Script object
    #[uniffi::constructor]
    pub fn new(electrum_url: String, tls: bool, validate_domain: bool) -> Result<Arc<Self>, Error> {
        let url = wollet::ElectrumUrl::new(&electrum_url, tls, validate_domain);
        let client = wollet::ElectrumClient::new(&url)?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
        }))
    }

    pub fn broadcast(&self, tx: &Transaction) -> Result<Arc<Txid>, Error> {
        Ok(Arc::new(self.inner.lock()?.broadcast(tx.as_ref())?.into()))
    }

    pub fn full_scan(&self, wollet: &Wollet) -> Result<Option<Arc<Update>>, Error> {
        let wollet = wollet.inner_wollet()?;
        let update: Option<wollet::Update> = self.inner.lock()?.full_scan(&wollet)?;
        Ok(update.map(Into::into).map(Arc::new))
    }
}

// TODO to be removed
#[derive(uniffi::Object, Debug, Clone)]
#[uniffi::export(Display)]
pub struct ElectrumUrl {
    pub(crate) url: String,
    pub(crate) tls: bool,
    pub(crate) validate_domain: bool,
}

impl Display for ElectrumUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[uniffi::export]
impl ElectrumUrl {
    /// Construct a Script object
    #[uniffi::constructor]
    pub fn new(electrum_url: String, tls: bool, validate_domain: bool) -> Arc<Self> {
        Arc::new(Self {
            url: electrum_url,
            tls,
            validate_domain,
        })
    }
}
