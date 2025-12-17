//! BIP21 URI parsing and access

use std::{fmt::Display, str::FromStr, sync::Arc};

use crate::LwkError;

/// A parsed Bitcoin BIP21 URI with optional parameters.
///
/// BIP21 URIs have the format: `bitcoin:<address>?amount=<amount>&label=<label>&message=<message>`
/// They can also include lightning parameters like `lightning=<bolt11>` or `lno=<bolt12>`.
#[derive(uniffi::Object)]
pub struct Bip21 {
    inner: lwk_payment_instructions::Bip21,
}

impl Display for Bip21 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<lwk_payment_instructions::Bip21> for Bip21 {
    fn from(inner: lwk_payment_instructions::Bip21) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl Bip21 {
    /// Parse a BIP21 URI string
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_payment_instructions::Bip21::from_str(s)
            .map_err(|e| LwkError::Generic { msg: e })?;
        Ok(Arc::new(Self { inner }))
    }

    /// Returns the original URI string
    pub fn as_str(&self) -> String {
        self.inner.as_str().to_string()
    }

    /// Returns the Bitcoin address from the BIP21 URI
    pub fn address(&self) -> Arc<crate::blockdata::address::BitcoinAddress> {
        Arc::new(self.inner.address().into())
    }

    /// Returns the amount in satoshis if present
    pub fn amount(&self) -> Option<u64> {
        self.inner.amount()
    }

    /// Returns the label if present
    pub fn label(&self) -> Option<String> {
        self.inner.label()
    }

    /// Returns the message if present
    pub fn message(&self) -> Option<String> {
        self.inner.message()
    }

    /// Returns the lightning BOLT11 invoice as a string if present
    #[cfg(feature = "lightning")]
    pub fn lightning(&self) -> Option<Arc<crate::Bolt11Invoice>> {
        self.inner
            .lightning()
            .and_then(|inv| crate::Bolt11Invoice::new(&inv.to_string()).ok())
    }

    /// Returns the BOLT12 offer as a string if present
    pub fn offer(&self) -> Option<String> {
        self.inner.offer().map(|o| o.to_string())
    }

    /// Returns the payjoin endpoint URL if present
    pub fn payjoin(&self) -> Option<String> {
        self.inner.payjoin().map(|u| u.to_string())
    }

    /// Returns whether payjoin output substitution is allowed (defaults to true if absent)
    pub fn payjoin_output_substitution(&self) -> bool {
        self.inner.payjoin_output_substitution()
    }

    /// Returns the silent payment address (BIP-352) if present
    pub fn silent_payment_address(&self) -> Option<String> {
        self.inner.silent_payment_address().map(|sp| sp.to_string())
    }
}
