use crate::types::AssetId;
use crate::{LwkError, PublicKey, Script};

use std::sync::{Arc, Mutex};

use elements::pset::Output;

/// PSET output (read-only)
#[derive(uniffi::Object, Debug)]
pub struct PsetOutput {
    inner: Output,
}

impl From<Output> for PsetOutput {
    fn from(inner: Output) -> Self {
        Self { inner }
    }
}

impl AsRef<Output> for PsetOutput {
    fn as_ref(&self) -> &Output {
        &self.inner
    }
}

#[uniffi::export]
impl PsetOutput {
    /// Get the script pubkey.
    pub fn script_pubkey(&self) -> Arc<Script> {
        Arc::new(self.inner.script_pubkey.clone().into())
    }

    /// Get the explicit amount, if set.
    pub fn amount(&self) -> Option<u64> {
        self.inner.amount
    }

    /// Get the explicit asset ID, if set.
    pub fn asset(&self) -> Option<AssetId> {
        self.inner.asset.map(Into::into)
    }

    /// Get the blinder index, if set.
    pub fn blinder_index(&self) -> Option<u32> {
        self.inner.blinder_index
    }
}

/// Builder for PSET outputs
#[derive(uniffi::Object, Debug)]
pub struct PsetOutputBuilder {
    /// Uses Mutex for in-place mutation. See [`crate::TxBuilder`] for rationale.
    inner: Mutex<Option<Output>>,
}

fn builder_consumed() -> LwkError {
    "PsetOutputBuilder already consumed".into()
}

impl AsRef<Mutex<Option<Output>>> for PsetOutputBuilder {
    fn as_ref(&self) -> &Mutex<Option<Output>> {
        &self.inner
    }
}

#[uniffi::export]
impl PsetOutputBuilder {
    /// Construct a PsetOutputBuilder with explicit asset and value.
    #[uniffi::constructor]
    pub fn new_explicit(
        script_pubkey: &Script,
        satoshi: u64,
        asset: AssetId,
        blinding_key: Option<Arc<PublicKey>>,
    ) -> Arc<Self> {
        let inner = Output {
            script_pubkey: script_pubkey.into(),
            amount: Some(satoshi),
            asset: Some(asset.into()),
            blinding_key: blinding_key.map(|k| k.as_ref().into()),
            ..Default::default()
        };
        Arc::new(Self {
            inner: Mutex::new(Some(inner)),
        })
    }

    /// Set the blinder index.
    pub fn blinder_index(&self, index: Option<u32>) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_consumed)?;
        inner.blinder_index = index;
        Ok(())
    }

    /// Build the PsetOutput, consuming the builder.
    pub fn build(&self) -> Result<Arc<PsetOutput>, LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_consumed)?;
        Ok(Arc::new(PsetOutput::from(inner)))
    }
}
