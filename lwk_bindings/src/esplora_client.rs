use std::sync::{Arc, Mutex};

use lwk_wollet::clients::blocking::{self, BlockchainBackend};

use crate::{LwkError, Network, Transaction, Txid, Update, Wollet};

/// Wrapper over [`blocking::EsploraClient`]
#[derive(uniffi::Object, Debug)]
pub struct EsploraClient {
    inner: Mutex<blocking::EsploraClient>,
}
#[derive(uniffi::Record)]
pub struct EsploraClientBuilder {
    base_url: String,
    network: Arc<Network>,
    #[uniffi(default = false)]
    waterfalls: bool,
    #[uniffi(default = None)]
    concurrency: Option<u32>,
    #[uniffi(default = None)]
    timeout: Option<u8>,
}

impl From<EsploraClientBuilder> for lwk_wollet::clients::EsploraClientBuilder {
    fn from(builder: EsploraClientBuilder) -> Self {
        let mut result = lwk_wollet::clients::EsploraClientBuilder::new(
            &builder.base_url,
            builder.network.as_ref().clone().into(),
        );
        if builder.waterfalls {
            result = result.waterfalls(true);
        }
        if let Some(concurrency) = builder.concurrency {
            result = result.concurrency(concurrency as usize);
        }
        if let Some(timeout) = builder.timeout {
            result = result.timeout(timeout);
        }
        result
    }
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

    #[uniffi::constructor]
    pub fn from_builder(builder: EsploraClientBuilder) -> Result<Arc<Self>, LwkError> {
        Ok(Arc::new(Self {
            inner: Mutex::new(
                lwk_wollet::clients::EsploraClientBuilder::from(builder).build_blocking()?,
            ),
        }))
    }

    pub fn broadcast(&self, tx: &Transaction) -> Result<Arc<Txid>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.broadcast(tx.as_ref())?.into()))
    }

    /// See [`BlockchainBackend::full_scan`]
    pub fn full_scan(&self, wollet: &Wollet) -> Result<Option<Arc<Update>>, LwkError> {
        self.full_scan_to_index(wollet, 0)
    }

    /// See [`BlockchainBackend::full_scan_to_index`]
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
