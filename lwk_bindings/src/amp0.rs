use std::fmt::Display;
use std::sync::{Arc, Mutex};

use lwk_common::Amp0Signer;

// use crate::{LwkError, Pset, WolletDescriptor};
use crate::{AddressResult, LwkError, Network, Pset, Signer, Transaction, WolletDescriptor};

/// Context for actions related to an AMP0 (sub)account
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

    /// AMP ID
    pub fn amp_id(&self) -> Result<String, LwkError> {
        Ok(self.inner.lock()?.amp_id().into())
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

    /// Ask AMP0 server to cosign
    pub fn sign(&self, pset: &Amp0Pset) -> Result<Arc<Transaction>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.sign(pset.as_ref())?.into()))
    }
}

/// A PSET to use with AMP0
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

/// Signer information necessary for full login to AMP0
#[derive(uniffi::Object, Clone)]
#[uniffi::export(Display)]
pub struct Amp0SignerData {
    inner: lwk_common::Amp0SignerData,
}

impl Display for Amp0SignerData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<lwk_common::Amp0SignerData> for Amp0SignerData {
    fn from(inner: lwk_common::Amp0SignerData) -> Self {
        Self { inner }
    }
}

impl AsRef<lwk_common::Amp0SignerData> for Amp0SignerData {
    fn as_ref(&self) -> &lwk_common::Amp0SignerData {
        &self.inner
    }
}

#[uniffi::export]
impl Signer {
    /// AMP0 signer data for login
    pub fn amp0_signer_data(&self) -> Result<Amp0SignerData, LwkError> {
        Ok(self.inner.amp0_signer_data()?.into())
    }

    /// AMP0 sign login challenge
    fn amp0_sign_challenge(&self, challenge: &str) -> Result<String, LwkError> {
        Ok(self.inner.amp0_sign_challenge(challenge)?)
    }

    /// AMP0 account xpub
    fn amp0_account_xpub(&self, account: u32) -> Result<String, LwkError> {
        Ok(self.inner.amp0_account_xpub(account)?.to_string())
    }
}

/// Session connecting to AMP0
#[derive(uniffi::Object)]
pub struct Amp0Connected {
    /// Uniffi doesn't allow to accept self and consume the parameter (everything is behind Arc)
    /// So, inside the Mutex we have an option that allow to consume the inner builder and also
    /// to emulate the consumption of this object after login.
    /// (same approach used for TxBuilder)
    inner: Mutex<Option<lwk_wollet::amp0::blocking::Amp0Connected>>,
}

/// Session logged in AMP0
#[derive(uniffi::Object)]
pub struct Amp0LoggedIn {
    inner: Mutex<lwk_wollet::amp0::blocking::Amp0LoggedIn>,
}

fn amp0_err() -> LwkError {
    "AMP0 session already logged in or it errored".into()
}

#[uniffi::export]
impl Amp0Connected {
    /// Connect and register to AMP0
    #[uniffi::constructor]
    pub fn new(network: &Network, signer_data: &Amp0SignerData) -> Result<Self, LwkError> {
        let inner = lwk_wollet::amp0::blocking::Amp0Connected::new(
            network.into(),
            signer_data.inner.clone(),
        )?;
        Ok(Amp0Connected {
            inner: Mutex::new(Some(inner)),
        })
    }

    /// Obtain a login challenge
    ///
    /// This must be signed with [`Signer::amp0_sign_challenge()`].
    pub fn get_challenge(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(amp0_err)?
            .get_challenge()?)
    }

    /// Log in
    ///
    /// `sig` must be obtained from [`Signer::amp0_sign_challenge()`] called with the value returned
    /// by [`Amp0Connected::get_challenge()`]
    pub fn login(self: Arc<Self>, sig: &str) -> Result<Arc<Amp0LoggedIn>, LwkError> {
        let mut lock = self.inner.lock()?;
        let amp0 = lock.take().ok_or_else(amp0_err)?;
        let amp0 = amp0.login(sig)?;
        Ok(Arc::new(Amp0LoggedIn {
            inner: Mutex::new(amp0),
        }))
    }
}

#[uniffi::export]
impl Amp0LoggedIn {
    /// List of AMP IDs.
    pub fn get_amp_ids(&self) -> Result<Vec<String>, LwkError> {
        Ok(self.inner.lock()?.get_amp_ids()?)
    }

    /// Get the next account for AMP0 account creation
    ///
    /// This must be given to [`Signer::amp0_account_xpub()`] to obtain the xpub to pass to
    /// [`Amp0LoggedIn::create_amp0_account()`]
    pub fn next_account(&self) -> Result<u32, LwkError> {
        Ok(self.inner.lock()?.next_account()?)
    }

    /// Create a new AMP0 account
    ///
    /// `account_xpub` must be obtained from [`Signer::amp0_account_xpub()`] called with the value obtained from
    /// [`Amp0LoggedIn::next_account()`]
    pub fn create_amp0_account(
        &self,
        pointer: u32,
        account_xpub: &str,
    ) -> Result<String, LwkError> {
        use elements::bitcoin::bip32::Xpub;
        use std::str::FromStr;
        let account_xpub = Xpub::from_str(account_xpub)?;
        Ok(self
            .inner
            .lock()?
            .create_amp0_account(pointer, &account_xpub)?)
    }

    /// Create a new Watch-Only entry for this wallet
    pub fn create_watch_only(&self, username: &str, password: &str) -> Result<(), LwkError> {
        Ok(self.inner.lock()?.create_watch_only(username, password)?)
    }
}
