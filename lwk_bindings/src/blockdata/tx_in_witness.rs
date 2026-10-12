//! Liquid transaction input witness

use crate::LwkError;
use std::sync::{Arc, Mutex};

/// A transaction input witness.
#[derive(uniffi::Object, Debug, Clone)]
pub struct TxInWitness {
    inner: elements::TxInWitness,
}

impl From<elements::TxInWitness> for TxInWitness {
    fn from(inner: elements::TxInWitness) -> Self {
        Self { inner }
    }
}

impl From<&TxInWitness> for elements::TxInWitness {
    fn from(value: &TxInWitness) -> Self {
        value.inner.clone()
    }
}

impl AsRef<elements::TxInWitness> for TxInWitness {
    fn as_ref(&self) -> &elements::TxInWitness {
        &self.inner
    }
}

#[uniffi::export]
impl TxInWitness {
    /// Create an empty witness.
    #[uniffi::constructor]
    pub fn empty() -> Arc<Self> {
        Arc::new(Self {
            inner: elements::TxInWitness::default(),
        })
    }

    /// Create a witness from script witness elements.
    #[uniffi::constructor]
    pub fn from_script_witness(script_witness: &[Vec<u8>]) -> Arc<Self> {
        Arc::new(Self {
            inner: elements::TxInWitness {
                script_witness: script_witness.to_vec().into(),
                ..Default::default()
            },
        })
    }

    /// Get the script witness elements.
    pub fn script_witness(&self) -> Vec<Vec<u8>> {
        self.inner.script_witness.to_vec()
    }

    /// Get the peg-in witness elements.
    pub fn pegin_witness(&self) -> Vec<Vec<u8>> {
        lwk_wollet::pegin::pegin_witness_to_elements(&self.inner.pegin_witness)
    }

    /// Check if the witness is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Builder for creating a TxInWitness.
#[derive(uniffi::Object, Debug)]
pub struct TxInWitnessBuilder {
    inner: Mutex<Option<elements::TxInWitness>>,
}

#[uniffi::export]
impl TxInWitnessBuilder {
    /// Create a new witness builder.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Some(elements::TxInWitness::default())),
        })
    }

    /// Set the script witness elements.
    pub fn script_witness(&self, witness: &[Vec<u8>]) -> Result<(), LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.as_mut().ok_or(LwkError::ObjectConsumed)?;
        inner.script_witness = witness.to_vec().into();
        Ok(())
    }

    /// Set the peg-in witness elements.
    pub fn pegin_witness(&self, witness: &[Vec<u8>]) -> Result<(), LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.as_mut().ok_or(LwkError::ObjectConsumed)?;
        inner.pegin_witness = lwk_wollet::pegin::pegin_witness_from_elements(witness)?;
        Ok(())
    }

    /// Set the amount rangeproof from serialized bytes.
    pub fn amount_rangeproof(&self, proof: &[u8]) -> Result<(), LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.as_mut().ok_or(LwkError::ObjectConsumed)?;
        inner.amount_rangeproof = elements::RangeProof::from_slice(proof)?;
        Ok(())
    }

    /// Set the inflation keys rangeproof from serialized bytes.
    pub fn inflation_keys_rangeproof(&self, proof: &[u8]) -> Result<(), LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.as_mut().ok_or(LwkError::ObjectConsumed)?;
        inner.inflation_keys_rangeproof = elements::RangeProof::from_slice(proof)?;
        Ok(())
    }

    /// Build the TxInWitness.
    pub fn build(&self) -> Result<Arc<TxInWitness>, LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(Arc::new(TxInWitness { inner }))
    }
}
