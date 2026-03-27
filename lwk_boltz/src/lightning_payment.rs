use std::{fmt::Display, str::FromStr};

use boltz_client::{lightning_invoice::ParseOrSemanticError, Bolt11Invoice};
use lightning::offers::{offer::Offer, parse::Bolt12ParseError};
use lnurl::lnurl::LnUrl;

use crate::Error;

#[derive(Debug, Clone)]
pub enum LightningPayment {
    Bolt11(Box<Bolt11Invoice>),
    Bolt12 {
        offer: Box<Offer>,

        /// This is the amount of the bolt12 invoice that is going to be created from this offer.
        invoice_amount: Option<u64>,
    },
    LnUrl(Box<LnUrl>),
}

impl FromStr for LightningPayment {
    type Err = (ParseOrSemanticError, Bolt12ParseError, lnurl::Error);

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Bolt11Invoice::from_str(s) {
            Ok(invoice) => Ok(LightningPayment::Bolt11(Box::new(invoice))),
            Err(e1) => match Offer::from_str(s) {
                Ok(offer) => Ok(LightningPayment::Bolt12 {
                    offer: Box::new(offer),
                    invoice_amount: None,
                }),
                Err(e2) => match LnUrl::from_str(s) {
                    Ok(lnurl) => Ok(LightningPayment::LnUrl(Box::new(lnurl))),
                    Err(e3) => Err((e1, e2, e3)),
                },
            },
        }
    }
}

impl Display for LightningPayment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LightningPayment::Bolt11(invoice) => write!(f, "{invoice}"),
            LightningPayment::Bolt12 {
                offer,
                invoice_amount: _,
            } => write!(f, "{offer}"),
            LightningPayment::LnUrl(lnurl) => write!(f, "{lnurl}"),
        }
    }
}

impl From<Bolt11Invoice> for LightningPayment {
    fn from(invoice: Bolt11Invoice) -> Self {
        LightningPayment::Bolt11(Box::new(invoice))
    }
}

impl LightningPayment {
    /// Returns the offer if this is a BOLT12 payment, None otherwise
    pub fn bolt12(&self) -> Option<&Offer> {
        match self {
            LightningPayment::Bolt12 { offer, .. } => Some(offer),
            _ => None,
        }
    }

    /// Returns the invoice amount in satoshis if this is a BOLT12 payment and it's present.
    /// Error if this isn't a Bolt12.
    pub fn bolt12_invoice_amount(&self) -> Result<Option<u64>, Error> {
        match self {
            LightningPayment::Bolt12 {
                offer: _,
                invoice_amount,
            } => Ok(invoice_amount.map(|msats| msats / 1000)),
            _ => Err(Error::ExpectedBolt12Variant),
        }
    }

    /// Sets the amount for a BOLT12 offer
    ///
    /// # Arguments
    ///
    /// * `amount` - The amount in satoshis for the BOLT12 invoice
    ///
    /// # Errors
    ///
    /// Returns an error if this is not a BOLT12 offer or if the Offer contains an amount
    pub fn set_bolt12_invoice_amount(&mut self, amount: u64) -> Result<(), Error> {
        match self {
            LightningPayment::Bolt12 {
                invoice_amount,
                offer,
                ..
            } => {
                if offer.amount().is_some() {
                    return Err(Error::Generic(
                        "Offer contains amount, specify number of items".to_string(),
                    ));
                }

                // Convert satoshis to millisatoshis for internal storage
                let amount_msats = amount
                    .checked_mul(1000)
                    .ok_or_else(|| Error::Generic("Amount overflow".to_string()))?;

                *invoice_amount = Some(amount_msats);
                Ok(())
            }
            _ => Err(Error::ExpectedBolt12Variant),
        }
    }

    /// Sets the amount for a BOLT12 offer based on the number of items
    ///
    /// This calculates the final amount as `items * offer_amount` where
    /// `offer_amount` is the amount specified in the offer (if any).
    ///
    /// # Arguments
    ///
    /// * `items` - The number of items to purchase
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - This is not a BOLT12 offer
    /// - The offer does not have an amount set
    pub fn set_bolt12_invoice_amount_via_items(&mut self, items: u64) -> Result<(), Error> {
        use lightning::offers::offer::Amount;

        match self {
            LightningPayment::Bolt12 {
                offer,
                invoice_amount,
            } => {
                // Get the per-item amount from the offer
                let offer_amount = match offer.amount() {
                    Some(Amount::Bitcoin { amount_msats }) => amount_msats,
                    Some(Amount::Currency { .. }) => {
                        return Err(Error::Generic("Currency amounts not supported".to_string()))
                    }
                    None => {
                        return Err(Error::InvoiceWithoutAmount(offer.to_string()));
                    }
                };

                // Calculate total amount
                let total_amount = items
                    .checked_mul(offer_amount)
                    .ok_or_else(|| Error::Generic("Amount overflow".to_string()))?;

                *invoice_amount = Some(total_amount);
                Ok(())
            }
            _ => Err(Error::ExpectedBolt12Variant),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        let invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let offer = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";
        let lnurl =
            "lnurl1dp68gurn8ghj7mn0wd68yene9e3k7mf0d3h82unvwqhkzurf9amrztmvde6hymp0xge7pp36";
        let payment = LightningPayment::from_str(invoice).unwrap();
        assert!(matches!(payment, LightningPayment::Bolt11(_)));
        assert_eq!(payment.to_string(), invoice);
        let payment = LightningPayment::from_str(offer).unwrap();
        assert!(matches!(payment, LightningPayment::Bolt12 { .. }));
        assert_eq!(payment.to_string(), offer);
        let payment = LightningPayment::from_str(lnurl).unwrap();
        assert!(matches!(payment, LightningPayment::LnUrl(_)));
        assert_eq!(payment.to_string(), lnurl);
        let err = "not a valid invoice or offer or lnurl";
        let err = LightningPayment::from_str(err).unwrap_err();
        assert!(matches!(err, (_, _, _)));
    }

    #[test]
    fn test_bolt12_methods_offer_without_amount() {
        let offer_without_amount = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";

        // Test bolt12() method with offer without amount
        let mut payment_without_amount = LightningPayment::from_str(offer_without_amount).unwrap();
        assert!(payment_without_amount.bolt12().is_some());
        assert!(matches!(
            payment_without_amount,
            LightningPayment::Bolt12 { .. }
        ));

        assert_eq!(
            payment_without_amount.bolt12_invoice_amount().unwrap(),
            None
        );

        // Test set_bolt12_invoice_amount()
        payment_without_amount
            .set_bolt12_invoice_amount(5000)
            .unwrap();
        assert_eq!(
            payment_without_amount.bolt12_invoice_amount().unwrap(),
            Some(5000)
        );

        let invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let mut payment_bolt11 = LightningPayment::from_str(invoice).unwrap();
        assert!(payment_bolt11.bolt12().is_none());
        assert!(payment_bolt11.set_bolt12_invoice_amount(5000).is_err());
        let offer_no_amount = payment_without_amount.bolt12().unwrap();
        assert!(offer_no_amount.amount().is_none());
    }

    #[test]
    fn test_bolt12_methods_offer_with_amount() {
        let offer_with_amount =
            "lno1pqpq86q2qahkuefqwdshg93pqtqft5rf2w8ed0c5chus7mqg2x7lx49qajrq8x3yhuu2w0msttwzc";

        // Test bolt12() method with offer with amount
        let payment_with_amount = LightningPayment::from_str(offer_with_amount).unwrap();
        assert!(payment_with_amount.bolt12().is_some());
        assert!(matches!(
            payment_with_amount,
            LightningPayment::Bolt12 { .. }
        ));
        assert_eq!(payment_with_amount.bolt12_invoice_amount().unwrap(), None);

        // Test set_bolt12_invoice_amount_via_items() with offer that has an amount
        let mut payment_items = LightningPayment::from_str(offer_with_amount).unwrap();
        payment_items
            .set_bolt12_invoice_amount_via_items(10)
            .unwrap();
        let amount = payment_items.bolt12_invoice_amount().unwrap();
        assert!(amount.is_some());

        // Test that bolt12() returns None for non-Bolt12 variants
        let invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let payment_bolt11 = LightningPayment::from_str(invoice).unwrap();
        assert!(payment_bolt11.bolt12().is_none());

        // Test that set_bolt12_invoice_amount() fails on non-Bolt12 variants
        let mut payment_bolt11 = LightningPayment::from_str(invoice).unwrap();
        assert!(payment_bolt11.set_bolt12_invoice_amount(5000).is_err());
    }
}
