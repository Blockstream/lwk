use crate::types::{AssetId, Hex, Tweak};
use crate::{Issuance, LwkError, OutPoint, Script, TxOut, TxSequence, Txid};

use std::sync::{Arc, Mutex};

use elements::pset::Input;

/// PSET input (read-only)
#[derive(uniffi::Object, Debug)]
pub struct PsetInput {
    inner: Input,
}

impl From<Input> for PsetInput {
    fn from(inner: Input) -> Self {
        Self { inner }
    }
}

impl PsetInput {
    pub(crate) fn from_inner(inner: Input) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl PsetInput {
    /// Prevout TXID of the input.
    pub fn previous_txid(&self) -> Arc<Txid> {
        Arc::new(self.inner.previous_txid.into())
    }

    /// Prevout vout of the input.
    pub fn previous_vout(&self) -> u32 {
        self.inner.previous_output_index
    }

    /// Prevout scriptpubkey of the input.
    pub fn previous_script_pubkey(&self) -> Option<Arc<Script>> {
        self.inner
            .witness_utxo
            .as_ref()
            .map(|txout| Arc::new(txout.script_pubkey.clone().into()))
    }

    /// Redeem script of the input.
    pub fn redeem_script(&self) -> Option<Arc<Script>> {
        self.inner
            .redeem_script
            .as_ref()
            .map(|s| Arc::new(s.clone().into()))
    }

    /// If the input has an issuance, the asset id.
    pub fn issuance_asset(&self) -> Option<AssetId> {
        self.inner
            .has_issuance()
            .then(|| self.inner.issuance_ids().0.into())
    }

    /// If the input has an issuance, the token id.
    pub fn issuance_token(&self) -> Option<AssetId> {
        self.inner
            .has_issuance()
            .then(|| self.inner.issuance_ids().1.into())
    }

    /// If the input has a (re)issuance, the issuance object.
    pub fn issuance(&self) -> Option<Arc<Issuance>> {
        self.inner
            .has_issuance()
            .then(|| Arc::new(lwk_common::Issuance::new(&self.inner).into()))
    }

    /// Input sighash.
    pub fn sighash(&self) -> u32 {
        self.inner.sighash_type.map(|s| s.to_u32()).unwrap_or(1)
    }
}

/// Builder for PSET inputs
#[derive(uniffi::Object, Debug)]
pub struct PsetInputBuilder {
    /// Uses Mutex for in-place mutation. See [`crate::TxBuilder`] for rationale.
    inner: Mutex<Option<Input>>,
}

fn builder_consumed() -> LwkError {
    "PsetInputBuilder already consumed".into()
}

impl AsRef<Mutex<Option<Input>>> for PsetInputBuilder {
    fn as_ref(&self) -> &Mutex<Option<Input>> {
        &self.inner
    }
}

#[uniffi::export]
impl PsetInputBuilder {
    /// Construct a PsetInputBuilder from an outpoint.
    #[uniffi::constructor]
    pub fn from_prevout(outpoint: &OutPoint) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Some(Input::from_prevout(outpoint.into()))),
        })
    }

    /// Set the witness UTXO.
    pub fn witness_utxo(&self, utxo: &TxOut) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_consumed)?;
        inner.witness_utxo = Some(utxo.into());
        Ok(())
    }

    /// Set the sequence number.
    pub fn sequence(&self, sequence: &TxSequence) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_consumed)?;
        inner.sequence = Some((*sequence).into());
        Ok(())
    }

    /// Set the issuance value amount.
    pub fn issuance_value_amount(&self, amount: u64) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_consumed)?;
        inner.issuance_value_amount = Some(amount);
        Ok(())
    }

    /// Set the issuance inflation keys.
    pub fn issuance_inflation_keys(&self, amount: u64) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_consumed)?;
        inner.issuance_inflation_keys = Some(amount);
        Ok(())
    }

    /// Set the issuance asset entropy.
    pub fn issuance_asset_entropy(&self, entropy: &Hex) -> Result<(), LwkError> {
        let bytes: [u8; 32] = entropy.as_ref().try_into()?;
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_consumed)?;
        inner.issuance_asset_entropy = Some(bytes);
        Ok(())
    }

    /// Set the blinded issuance flag.
    pub fn blinded_issuance(&self, flag: u8) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_consumed)?;
        inner.blinded_issuance = Some(flag);
        Ok(())
    }

    /// Set the issuance blinding nonce.
    pub fn issuance_blinding_nonce(&self, nonce: &Tweak) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.as_mut().ok_or_else(builder_consumed)?;
        inner.issuance_blinding_nonce = Some(nonce.into());
        Ok(())
    }

    /// Build the PsetInput, consuming the builder.
    pub fn build(&self) -> Result<Arc<PsetInput>, LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_consumed)?;
        Ok(Arc::new(PsetInput::from_inner(inner)))
    }
}
