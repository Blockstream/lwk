//! Payment instructions parsing and categorization

use std::{fmt::Display, str::FromStr, sync::Arc};

use crate::{blockdata::address::BitcoinAddress, types::AssetId, Address, LwkError};

/// The kind/type of a payment category without the associated data
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentKind {
    /// A Bitcoin address
    BitcoinAddress,
    /// A Liquid address
    LiquidAddress,
    /// A Lightning BOLT11 invoice
    LightningInvoice,
    /// A Lightning BOLT12 offer
    LightningOffer,
    /// An LNURL
    LnUrl,
    /// A BIP353 payment instruction (₿user@domain)
    Bip353,
    /// A BIP21 URI
    Bip21,
    /// A Liquid BIP21 URI with amount and asset
    LiquidBip21,
}

impl From<lwk_payment_instructions::PaymentKind> for PaymentKind {
    fn from(kind: lwk_payment_instructions::PaymentKind) -> Self {
        match kind {
            lwk_payment_instructions::PaymentKind::BitcoinAddress => PaymentKind::BitcoinAddress,
            lwk_payment_instructions::PaymentKind::LiquidAddress => PaymentKind::LiquidAddress,
            lwk_payment_instructions::PaymentKind::LightningInvoice => {
                PaymentKind::LightningInvoice
            }
            lwk_payment_instructions::PaymentKind::LightningOffer => PaymentKind::LightningOffer,
            lwk_payment_instructions::PaymentKind::LnUrl => PaymentKind::LnUrl,
            lwk_payment_instructions::PaymentKind::Bip353 => PaymentKind::Bip353,
            lwk_payment_instructions::PaymentKind::Bip21 => PaymentKind::Bip21,
            lwk_payment_instructions::PaymentKind::LiquidBip21 => PaymentKind::LiquidBip21,
            _ => unreachable!("Unknown PaymentCategoryKind variant"),
        }
    }
}

/// Liquid BIP21 payment details
#[derive(uniffi::Record, Clone)]
pub struct LiquidBip21 {
    /// The Liquid address
    pub address: Arc<Address>,
    /// The asset identifier
    pub asset: AssetId,
    /// The amount in satoshis
    pub amount: u64,
}

/// A parsed payment category from a payment instruction string.
///
/// This can be a Bitcoin address, Liquid address, Lightning invoice,
/// Lightning offer, LNURL, BIP353, BIP21 URI, or Liquid BIP21 URI.
#[derive(uniffi::Object)]
pub struct Payment {
    /// The original input string
    input: String,
    /// The parsed kind
    kind: PaymentKind,
}

impl Display for Payment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.input)
    }
}

#[uniffi::export]
impl Payment {
    /// Parse a payment instruction string into a PaymentCategory
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let parsed = lwk_payment_instructions::Payment::from_str(s)
            .map_err(|e| LwkError::Generic { msg: e })?;
        let kind = parsed.kind().into();
        Ok(Arc::new(Self {
            input: s.to_string(),
            kind,
        }))
    }

    /// Returns the kind of payment category
    pub fn kind(&self) -> PaymentKind {
        self.kind
    }

    /// Returns the Bitcoin address if this is a BitcoinAddress category, None otherwise
    ///
    /// Returns the address portion of the original input string
    pub fn bitcoin_address(&self) -> Option<Arc<BitcoinAddress>> {
        let parsed = lwk_payment_instructions::Payment::from_str(&self.input).ok()?;
        parsed
            .bitcoin_address()
            .map(|addr| Arc::new(addr.clone().into()))
    }

    /// Returns the Liquid address if this is a LiquidAddress category, None otherwise
    pub fn liquid_address(&self) -> Option<Arc<Address>> {
        let parsed = lwk_payment_instructions::Payment::from_str(&self.input).ok()?;
        parsed
            .liquid_address()
            .map(|addr| Arc::new(Address::from(addr.clone())))
    }

    /// Returns the Lightning invoice as a string if this is a LightningInvoice category, None otherwise
    pub fn lightning_invoice(&self) -> Option<String> {
        let parsed = lwk_payment_instructions::Payment::from_str(&self.input).ok()?;
        parsed.lightning_invoice().map(|inv| inv.to_string())
    }

    /// Returns the Lightning offer as a string if this is a LightningOffer category, None otherwise
    pub fn lightning_offer(&self) -> Option<String> {
        let parsed = lwk_payment_instructions::Payment::from_str(&self.input).ok()?;
        parsed.lightning_offer().map(|offer| offer.to_string())
    }

    /// Returns the LNURL as a string if this is an LnUrl category, None otherwise
    pub fn lnurl(&self) -> Option<String> {
        let parsed = lwk_payment_instructions::Payment::from_str(&self.input).ok()?;
        parsed.lnurl().map(|lnurl| lnurl.to_string())
    }

    /// Returns the BIP353 address (without the ₿ prefix) if this is a Bip353 category, None otherwise
    pub fn bip353(&self) -> Option<String> {
        let parsed = lwk_payment_instructions::Payment::from_str(&self.input).ok()?;
        parsed.bip353().map(|s| s.to_string())
    }

    /// Returns the BIP21 URI as a string if this is a Bip21 category, None otherwise
    ///
    /// Returns the original input string since it was parsed as a BIP21 URI
    pub fn bip21(&self) -> Option<String> {
        let parsed = lwk_payment_instructions::Payment::from_str(&self.input).ok()?;
        parsed.bip21().map(|_| self.input.clone())
    }

    /// Returns the Liquid BIP21 details if this is a LiquidBip21 category, None otherwise
    pub fn liquid_bip21(&self) -> Option<LiquidBip21> {
        let parsed = lwk_payment_instructions::Payment::from_str(&self.input).ok()?;
        parsed.liquid_bip21().map(|bip21| LiquidBip21 {
            address: Arc::new(Address::from(bip21.address.clone())),
            asset: bip21.asset.into(),
            amount: bip21.amount,
        })
    }

    /// Returns a LightningPayment if this category is payable via Lightning
    ///
    /// Returns Some for LightningInvoice, LightningOffer, and LnUrl categories.
    /// The returned LightningPayment can be used with BoltzSession::prepare_pay().
    #[cfg(feature = "lightning")]
    pub fn lightning_payment(&self) -> Option<Arc<crate::LightningPayment>> {
        match self.kind {
            PaymentKind::LightningInvoice | PaymentKind::LightningOffer | PaymentKind::LnUrl => {
                // Extract the payment string (strip schema prefix if present)
                let payment_str = self
                    .input
                    .strip_prefix("lightning:")
                    .or_else(|| self.input.strip_prefix("LIGHTNING:"))
                    .unwrap_or(&self.input);
                crate::LightningPayment::new(payment_str).ok()
            }
            _ => None,
        }
    }
}
