use std::{fmt::Display, sync::Mutex};

use crate::{LwkError, Network};

#[derive(uniffi::Object, Debug)]
#[uniffi::export(Display)]
pub struct TxBuilder {
    /// Uniffi doesn't allow to accept self and consume the parameter (everything is behind Arc)
    /// So, inside the Mutex we have an option because the inner builder consume self. Taking out the
    /// content of the option allow us to consume the object. However we must ensure that every
    /// method will have Some on finish, so that every other method could take and unwrap.
    inner: Mutex<Option<lwk_wollet::TxBuilder>>,
}

impl Display for TxBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.inner.lock() {
            Ok(r) => write!(f, "{:?}", r.as_ref().expect("must be some")),
            Err(_) => write!(f, "{:?}", self.inner),
        }
    }
}

#[uniffi::export]
impl TxBuilder {
    /// Construct a transaction builder
    #[uniffi::constructor]
    pub fn new(network: &Network) -> Self {
        TxBuilder {
            inner: Mutex::new(Some(lwk_wollet::TxBuilder::new((*network).into()))),
        }
    }

    /// Set the fee rate
    pub fn fee_rate(&self, rate: Option<f32>) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().expect("must be some");
        *lock = Some(inner.fee_rate(rate));
        Ok(())
    }
}
