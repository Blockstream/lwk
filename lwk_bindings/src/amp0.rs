use std::sync::{Arc, Mutex};

// use crate::{LwkError, Pset, WolletDescriptor};
use crate::{AddressResult, LwkError, Network, Pset, Txid, WolletDescriptor};

/// Wrapper over [`lwk_wollet::amp0::blocking::Amp0`]
#[derive(uniffi::Object)]
pub struct Amp0 {
    inner: Mutex<lwk_wollet::amp0::blocking::Amp0>,
}

#[uniffi::export]
impl Amp0 {
    /// Construct an AMP0 context
    #[uniffi::constructor]
    pub fn new(
        network: &Network,
        username: &str,
        password: &str,
        amp_id: &str,
    ) -> Result<Self, LwkError> {
        let inner =
            lwk_wollet::amp0::blocking::Amp0::new(network.into(), username, password, amp_id)?;
        Ok(Self {
            inner: Mutex::new(inner),
        })
    }

    /// Index of the last returned address
    pub fn last_index(&self) -> Result<u32, LwkError> {
        Ok(self.inner.lock()?.last_index())
    }

    /// Wollet descriptor
    pub fn wollet_descriptor(&self) -> Result<Arc<WolletDescriptor>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.wollet_descriptor().into()))
    }

    /// Get an address
    ///
    /// If `index` is None, a new address is returned.
    pub fn address(&self, index: Option<u32>) -> Result<Arc<AddressResult>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.address(index)?.into()))
    }

    /// Ask AMP0 server to cosign and broadcast the transaction
    pub fn send(&self, pset: &Pset) -> Result<Arc<Txid>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.send(pset.as_ref())?.into()))
    }
}
