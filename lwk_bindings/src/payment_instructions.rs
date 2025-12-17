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
    /// A BIP321 URI (BIP21 without address but with payment method)
    Bip321,
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
            lwk_payment_instructions::PaymentKind::Bip321 => PaymentKind::Bip321,
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
    inner: lwk_payment_instructions::Payment,
}

impl Display for Payment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use lwk_payment_instructions::Payment as P;
        match &self.inner {
            P::BitcoinAddress(addr) => write!(f, "{}", addr.clone().assume_checked()),
            P::LiquidAddress(addr) => write!(f, "{addr}"),
            P::LightningInvoice(invoice) => write!(f, "{invoice}"),
            P::LightningOffer(offer) => write!(f, "{offer}"),
            P::LnUrlCat(lnurl) => write!(f, "{lnurl}"),
            P::Bip353(s) => write!(f, "{s}"),
            P::Bip21(s) => write!(f, "{s}"),
            P::Bip321(s) => write!(f, "{s}"),
            P::LiquidBip21(bip21) => write!(f, "{}", bip21.address),
            _ => write!(f, "{:?}", self.inner),
        }
    }
}

#[uniffi::export]
impl Payment {
    /// Parse a payment instruction string into a PaymentCategory
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_payment_instructions::Payment::from_str(s)
            .map_err(|e| LwkError::Generic { msg: e })?;
        Ok(Arc::new(Self { inner }))
    }

    /// Returns the kind of payment category
    pub fn kind(&self) -> PaymentKind {
        self.inner.kind().into()
    }

    /// Returns the Bitcoin address if this is a BitcoinAddress category, None otherwise
    ///
    /// Returns the address portion of the original input string
    pub fn bitcoin_address(&self) -> Option<Arc<BitcoinAddress>> {
        self.inner
            .bitcoin_address()
            .map(|addr| Arc::new(addr.clone().into()))
    }

    /// Returns the Liquid address if this is a LiquidAddress category, None otherwise
    pub fn liquid_address(&self) -> Option<Arc<Address>> {
        self.inner
            .liquid_address()
            .map(|addr| Arc::new(Address::from(addr.clone())))
    }

    /// Returns the Lightning invoice if this is a `LightningInvoice` category, `None` otherwise
    #[cfg(feature = "lightning")]
    pub fn lightning_invoice(&self) -> Option<Arc<crate::Bolt11Invoice>> {
        self.inner
            .lightning_invoice()
            .and_then(|inv| crate::Bolt11Invoice::new(&inv.to_string()).ok())
    }

    /// Returns the Lightning offer as a string if this is a LightningOffer category, None otherwise
    pub fn lightning_offer(&self) -> Option<String> {
        self.inner.lightning_offer().map(|offer| offer.to_string())
    }

    /// Returns the LNURL as a string if this is an LnUrl category, None otherwise
    pub fn lnurl(&self) -> Option<String> {
        self.inner.lnurl().map(|lnurl| lnurl.to_string())
    }

    /// Returns the BIP353 address (without the ₿ prefix) if this is a Bip353 category, None otherwise
    pub fn bip353(&self) -> Option<String> {
        self.inner.bip353().map(|s| s.to_string())
    }

    /// Returns the BIP21 URI if this is a Bip21 category, None otherwise
    pub fn bip21(&self) -> Option<Arc<crate::bip21::Bip21>> {
        self.inner
            .bip21()
            .map(|bip21| Arc::new(crate::bip21::Bip21::from(bip21.clone())))
    }

    /// Returns the BIP321 URI if this is a Bip321 category, None otherwise
    pub fn bip321(&self) -> Option<Arc<crate::bip321::Bip321>> {
        self.inner
            .bip321()
            .map(|bip321| Arc::new(crate::bip321::Bip321::from(bip321.clone())))
    }

    /// Returns the Liquid BIP21 details if this is a LiquidBip21 category, None otherwise
    pub fn liquid_bip21(&self) -> Option<LiquidBip21> {
        self.inner.liquid_bip21().map(|bip21| LiquidBip21 {
            address: Arc::new(Address::from(bip21.address.clone())),
            asset: bip21.asset.into(),
            amount: bip21.amount,
        })
    }

    /// Returns a `LightningPayment`` if this category is payable via Lightning
    ///
    /// Returns `Some` for `LightningInvoice`, `LightningOffer`, and `LnUrl` categories.
    /// The returned `LightningPayment` can be used with `BoltzSession::prepare_pay()`.
    #[cfg(feature = "lightning")]
    pub fn lightning_payment(&self) -> Option<Arc<crate::LightningPayment>> {
        use lwk_payment_instructions::Payment as P;
        match &self.inner {
            P::LightningInvoice(invoice) => crate::LightningPayment::new(&invoice.to_string()).ok(),
            P::LightningOffer(offer) => crate::LightningPayment::new(&offer.to_string()).ok(),
            P::LnUrlCat(lnurl) => crate::LightningPayment::new(&lnurl.to_string()).ok(),
            _ => None,
        }
    }
}
