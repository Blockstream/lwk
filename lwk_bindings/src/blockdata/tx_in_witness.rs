//! Liquid transaction input witness

use crate::{types::Hex, LwkError};
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
    pub fn from_script_witness(script_witness: Vec<Hex>) -> Arc<Self> {
        let witness: Vec<Vec<u8>> = script_witness
            .into_iter()
            .map(|h| h.as_ref().to_vec())
            .collect();
        Arc::new(Self {
            inner: elements::TxInWitness {
                script_witness: witness,
                ..Default::default()
            },
        })
    }

    /// Get the script witness elements.
    pub fn script_witness(&self) -> Vec<Hex> {
        self.inner
            .script_witness
            .iter()
            .map(|v| Hex::from(v.as_slice()))
            .collect()
    }

    /// Get the peg-in witness elements.
    pub fn pegin_witness(&self) -> Vec<Hex> {
        self.inner
            .pegin_witness
            .iter()
            .map(|v| Hex::from(v.as_slice()))
            .collect()
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
    pub fn script_witness(&self, witness: Vec<Hex>) -> Result<(), LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.as_mut().ok_or(LwkError::ObjectConsumed)?;
        inner.script_witness = witness.into_iter().map(|h| h.as_ref().to_vec()).collect();
        Ok(())
    }

    /// Set the peg-in witness elements.
    pub fn pegin_witness(&self, witness: Vec<Hex>) -> Result<(), LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.as_mut().ok_or(LwkError::ObjectConsumed)?;
        inner.pegin_witness = witness.into_iter().map(|h| h.as_ref().to_vec()).collect();
        Ok(())
    }

    /// Set the amount rangeproof from serialized bytes.
    pub fn amount_rangeproof(&self, proof: &Hex) -> Result<(), LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.as_mut().ok_or(LwkError::ObjectConsumed)?;
        let rangeproof = elements::secp256k1_zkp::RangeProof::from_slice(proof.as_ref())?;
        inner.amount_rangeproof = Some(Box::new(rangeproof));
        Ok(())
    }

    /// Set the inflation keys rangeproof from serialized bytes.
    pub fn inflation_keys_rangeproof(&self, proof: &Hex) -> Result<(), LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.as_mut().ok_or(LwkError::ObjectConsumed)?;
        let rangeproof = elements::secp256k1_zkp::RangeProof::from_slice(proof.as_ref())?;
        inner.inflation_keys_rangeproof = Some(Box::new(rangeproof));
        Ok(())
    }

    /// Build the TxInWitness.
    pub fn build(&self) -> Result<Arc<TxInWitness>, LwkError> {
        let mut guard = self.inner.lock()?;
        let inner = guard.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(Arc::new(TxInWitness { inner }))
    }
}
