use std::{fmt::Display, sync::Arc};

use crate::{Error, Transaction, Txid};

#[derive(uniffi::Object, Debug)]
pub struct ElectrumClient {
    inner: wollet::ElectrumClient,
}

#[uniffi::export]
impl ElectrumClient {
    /// Construct a Script object
    #[uniffi::constructor]
    pub fn new(electrum_url: String, tls: bool, validate_domain: bool) -> Result<Arc<Self>, Error> {
        let url = wollet::ElectrumUrl::new(&electrum_url, tls, validate_domain);
        let inner = wollet::ElectrumClient::new(&url)?;
        Ok(Arc::new(Self { inner }))
    }

    pub fn broadcast(&self, tx: &Transaction) -> Result<Arc<Txid>, Error> {
        Ok(Arc::new(self.inner.broadcast(tx.as_ref())?.into()))
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
