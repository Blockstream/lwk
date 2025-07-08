use crate::{
    types::{AssetId, SecretKey},
    LwkError, Script, TxOutSecrets,
};
use std::sync::Arc;

#[derive(uniffi::Object, Debug)]
pub struct TxOut {
    inner: elements::TxOut,
}

impl From<elements::TxOut> for TxOut {
    fn from(inner: elements::TxOut) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl TxOut {
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

    /// Unblind the output
    pub fn unblind(&self, secret_key: &SecretKey) -> Result<TxOutSecrets, LwkError> {
        Ok(self
            .inner
            .unblind(&lwk_wollet::EC, secret_key.into())
            .map(Into::into)?)
    }
}
