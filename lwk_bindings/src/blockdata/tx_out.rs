//! Liquid transaction output

use crate::{
    types::{AssetId, SecretKey},
    Address, LwkError, Network, Script, TxOutSecrets,
};
use elements::pset::serialize::Deserialize;
use elements::{confidential, TxOutWitness};
use std::sync::Arc;

/// A transaction output.
#[derive(uniffi::Object, Debug, Clone)]
pub struct TxOut {
    inner: elements::TxOut,
}

impl From<elements::TxOut> for TxOut {
    fn from(inner: elements::TxOut) -> Self {
        Self { inner }
    }
}

impl AsRef<elements::TxOut> for TxOut {
    fn as_ref(&self) -> &elements::TxOut {
        &self.inner
    }
}

impl From<TxOut> for elements::TxOut {
    fn from(value: TxOut) -> Self {
        value.inner
    }
}

impl From<&TxOut> for elements::TxOut {
    fn from(value: &TxOut) -> Self {
        value.inner.clone()
    }
}

#[uniffi::export]
impl TxOut {
    /// Create a TxOut with explicit asset and value from script pubkey hex, asset ID hex, and value.
    ///
    /// This is useful for constructing UTXOs for Simplicity transaction signing.
    #[uniffi::constructor]
    pub fn from_explicit(
        script_pubkey_hex: &str,
        asset_id_hex: &str,
        value: u64,
    ) -> Result<Arc<Self>, LwkError> {
        use elements::hex::FromHex;
        use std::str::FromStr;

        let script_bytes =
            Vec::<u8>::from_hex(script_pubkey_hex).map_err(|e| LwkError::Generic {
                msg: format!("Invalid script pubkey hex: {e}"),
            })?;
        let script_pubkey =
            elements::Script::deserialize(&script_bytes).map_err(|e| LwkError::Generic {
                msg: format!("Invalid script: {e}"),
            })?;

        let asset_id =
            elements::AssetId::from_str(asset_id_hex).map_err(|e| LwkError::Generic {
                msg: format!("Invalid asset ID: {e}"),
            })?;

        let inner = elements::TxOut {
            script_pubkey,
            asset: confidential::Asset::Explicit(asset_id),
            value: confidential::Value::Explicit(value),
            nonce: confidential::Nonce::Null,
            witness: TxOutWitness::default(),
        };

        Ok(Arc::new(Self { inner }))
    }

    /// Scriptpubkey
    pub fn script_pubkey(&self) -> Arc<Script> {
        let spk = self.inner.script_pubkey.clone().into();
        Arc::new(spk)
    }

    /// Whether or not this output is a fee output
    pub fn is_fee(&self) -> bool {
        self.inner.is_fee()
    }

    /// Returns if at least some part of this output are blinded
    pub fn is_partially_blinded(&self) -> bool {
        self.inner.is_partially_blinded()
    }

    /// If explicit returns the asset, if confidential [None]
    pub fn asset(&self) -> Option<AssetId> {
        self.inner.asset.explicit().map(Into::into)
    }

    /// If explicit returns the value, if confidential [None]
    pub fn value(&self) -> Option<u64> {
        self.inner.value.explicit()
    }

    /// Unconfidential address
    pub fn unconfidential_address(&self, network: &Network) -> Option<Arc<Address>> {
        elements::Address::from_script(
            &self.inner.script_pubkey,
            None,
            network.inner.address_params(),
        )
        .map(|a| Arc::new(a.into()))
    }

    /// Unblind the output
    pub fn unblind(&self, secret_key: &SecretKey) -> Result<TxOutSecrets, LwkError> {
        Ok(self
            .inner
            .unblind(&lwk_wollet::EC, secret_key.into())
            .map(Into::into)?)
    }
}
