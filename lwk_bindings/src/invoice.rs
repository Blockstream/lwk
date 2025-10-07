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

impl AsRef<lwk_boltz::Bolt11Invoice> for Bolt11Invoice {
    fn as_ref(&self) -> &lwk_boltz::Bolt11Invoice {
        &self.inner
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

    /// Returns the amount in millisatoshis if present, None if it's an "any amount" invoice
    pub fn amount_milli_satoshis(&self) -> Option<u64> {
        self.inner.amount_milli_satoshis()
    }

    /// Returns the payment hash as a hex string
    pub fn payment_hash(&self) -> String {
        format!("{}", self.inner.payment_hash())
    }

    /// Returns the invoice description as a string
    pub fn description(&self) -> String {
        format!("{}", self.inner.description())
    }

    /// Returns the payee's public key if present as a hex string
    pub fn payee_pub_key(&self) -> Option<String> {
        self.inner.payee_pub_key().map(|pk| pk.to_string())
    }

    /// Returns the invoice timestamp as seconds since Unix epoch
    pub fn timestamp(&self) -> u64 {
        self.inner.duration_since_epoch().as_secs()
    }

    /// Returns the expiry time in seconds (default is 3600 seconds / 1 hour if not specified)
    pub fn expiry_time(&self) -> u64 {
        self.inner.expiry_time().as_secs()
    }

    /// Returns the minimum CLTV expiry delta
    pub fn min_final_cltv_expiry_delta(&self) -> u64 {
        self.inner.min_final_cltv_expiry_delta()
    }

    /// Returns the network (bitcoin, testnet, signet, regtest)
    pub fn network(&self) -> String {
        format!("{:?}", self.inner.network())
    }

    /// Returns the payment secret as a debug string
    pub fn payment_secret(&self) -> String {
        format!("{:?}", self.inner.payment_secret())
    }
}
