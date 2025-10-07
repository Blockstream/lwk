use std::{fmt::Display, str::FromStr, sync::Arc};

use crate::LwkError;

/// Represents a syntactically and semantically correct lightning BOLT11 invoice.
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone)]
#[uniffi::export(Display)]
pub struct Bolt11Invoice {
    pub(crate) inner: lwk_boltz::Bolt11Invoice,
}

impl From<lwk_boltz::Bolt11Invoice> for Bolt11Invoice {
    fn from(inner: lwk_boltz::Bolt11Invoice) -> Self {
        Self { inner }
    }
}

impl From<Bolt11Invoice> for lwk_boltz::Bolt11Invoice {
    fn from(invoice: Bolt11Invoice) -> Self {
        invoice.inner
    }
}

impl From<&Bolt11Invoice> for lwk_boltz::Bolt11Invoice {
    fn from(invoice: &Bolt11Invoice) -> Self {
        invoice.inner.clone()
    }
}

impl Display for Bolt11Invoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl Bolt11Invoice {
    /// Construct a Bolt11Invoice from a string
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_boltz::Bolt11Invoice::from_str(s).map_err(|e| lwk_boltz::Error::from(e))?;
        Ok(Arc::new(Self { inner }))
    }
}
