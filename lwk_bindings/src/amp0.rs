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

/// Wrapper over [`lwk_wollet::amp0::Amp0Pset`]
#[derive(uniffi::Object)]
pub struct Amp0Pset {
    inner: lwk_wollet::amp0::Amp0Pset,
}

impl From<lwk_wollet::amp0::Amp0Pset> for Amp0Pset {
    fn from(inner: lwk_wollet::amp0::Amp0Pset) -> Self {
        Self { inner }
    }
}

impl AsRef<lwk_wollet::amp0::Amp0Pset> for Amp0Pset {
    fn as_ref(&self) -> &lwk_wollet::amp0::Amp0Pset {
        &self.inner
    }
}

#[uniffi::export]
impl Amp0Pset {
    /// Construct a PSET to use with AMP0
    #[uniffi::constructor]
    pub fn new(pset: &Pset, blinding_nonces: Vec<String>) -> Result<Arc<Self>, LwkError> {
        let pset = pset.as_ref().clone();
        let inner = lwk_wollet::amp0::Amp0Pset::new(pset, blinding_nonces)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Get the PSET
    pub fn pset(&self) -> Result<Pset, LwkError> {
        let pset = self.inner.pset().clone();
        Ok(pset.into())
    }

    /// Get blinding nonces
    pub fn blinding_nonces(&self) -> Result<Vec<String>, LwkError> {
        Ok(self.inner.blinding_nonces().to_vec())
    }
}
