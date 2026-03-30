use std::{
    fmt::Display,
    str::FromStr,
    sync::{Arc, Mutex},
};

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
        let inner = lwk_boltz::Bolt11Invoice::from_str(s).map_err(lwk_boltz::Error::from)?;
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
    pub fn invoice_description(&self) -> String {
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

/// Represents a lightning payment (bolt11 invoice or bolt12 offer)
#[derive(uniffi::Object)]
pub struct LightningPayment {
    inner: Mutex<lwk_boltz::LightningPayment>,
}

impl LightningPayment {
    pub(crate) fn clone(&self) -> Result<lwk_boltz::LightningPayment, LwkError> {
        Ok((*self.inner.lock()?).clone())
    }
}

impl From<lwk_boltz::LightningPayment> for LightningPayment {
    fn from(inner: lwk_boltz::LightningPayment) -> Self {
        Self {
            inner: Mutex::new(inner),
        }
    }
}

impl Display for LightningPayment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.lock().map_err(|_| std::fmt::Error)?;
        write!(f, "{}", *inner)
    }
}

#[uniffi::export]
impl LightningPayment {
    /// Construct a lightning payment (bolt11 invoice or bolt12 offer) from a string
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner =
            lwk_boltz::LightningPayment::from_str(s).map_err(|(e1, e2, e3)| LwkError::Generic {
                msg: format!("Failed to create lightning payment: {e1:?}, {e2:?}, {e3:?}"),
            })?;
        Ok(Arc::new(Self {
            inner: Mutex::new(inner),
        }))
    }

    /// Construct a lightning payment (bolt11 invoice or bolt12 offer) from a bolt11 invoice
    #[uniffi::constructor]
    pub fn from_bolt11_invoice(invoice: Arc<Bolt11Invoice>) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(lwk_boltz::LightningPayment::Bolt11(Box::new(
                invoice.as_ref().clone().into(),
            ))),
        })
    }

    /// Returns the bolt11 invoice if the lightning payment is a bolt11 invoice
    pub fn bolt11_invoice(&self) -> Result<Option<Arc<Bolt11Invoice>>, LwkError> {
        let inner = self.inner.lock()?;
        Ok(match &*inner {
            lwk_boltz::LightningPayment::Bolt11(invoice) => {
                Some(Arc::new(Bolt11Invoice::from((**invoice).clone())))
            }
            lwk_boltz::LightningPayment::Bolt12 { .. } => None,
            lwk_boltz::LightningPayment::LnUrl(_) => None,
        })
    }

    /// Returns true if this is a BOLT12 offer
    pub fn is_bolt12(&self) -> Result<bool, LwkError> {
        let inner = self.inner.lock()?;
        Ok(inner.bolt12().is_some())
    }

    /// Returns the invoice amount in satoshis for a BOLT12 offer if set
    ///
    /// Returns an error if this is not a BOLT12 offer
    pub fn bolt12_invoice_amount(&self) -> Result<Option<u64>, LwkError> {
        let inner = self.inner.lock()?;
        inner.bolt12_invoice_amount().map_err(Into::into)
    }

    /// Sets the amount for a BOLT12 offer without an amount
    ///
    /// The amount should be in satoshis.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - This is not a BOLT12 offer
    /// - The offer already has an amount (use set_bolt12_invoice_amount_via_items instead)
    pub fn set_bolt12_invoice_amount(&self, amount_sats: u64) -> Result<(), LwkError> {
        let mut inner = self.inner.lock()?;
        inner.set_bolt12_invoice_amount(amount_sats)?;
        Ok(())
    }

    /// Sets the amount for a BOLT12 offer based on number of items
    ///
    /// This calculates the final amount as `items * offer_amount` where
    /// `offer_amount` is the per-item amount specified in the offer.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - This is not a BOLT12 offer
    /// - The offer does not have an amount set
    pub fn set_bolt12_invoice_amount_via_items(&self, items: u64) -> Result<(), LwkError> {
        let mut inner = self.inner.lock()?;
        inner.set_bolt12_invoice_amount_via_items(items)?;
        Ok(())
    }

    /// Checks if the BOLT12 offer has an amount
    ///
    /// Returns true if the offer has a per-item amount (requires specifying number of items),
    /// false if it doesn't (requires specifying total amount in sats).
    ///
    /// Returns an error if this is not a BOLT12 offer.
    pub fn bolt12_offer_has_amount(&self) -> Result<bool, LwkError> {
        let inner = self.inner.lock()?;
        match &*inner {
            lwk_boltz::LightningPayment::Bolt12 { offer, .. } => Ok(offer.amount().is_some()),
            _ => Err(LwkError::Generic {
                msg: "Not a BOLT12 offer".to_string(),
            }),
        }
    }
}
