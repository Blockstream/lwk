use std::fmt;

use boltz_client::Bolt11Invoice;
use lightning::offers::invoice::Bolt12Invoice;

use crate::Error;

/// An enum representing either a BOLT11 or BOLT12 invoice
#[derive(Clone, Debug)]
pub enum Invoice {
    /// A BOLT11 invoice
    Bolt11(Box<Bolt11Invoice>),
    /// A BOLT12 invoice
    Bolt12(Box<Bolt12Invoice>),
}

impl Invoice {
    /// Get the amount in millisatoshis from the invoice
    ///
    /// Returns an error if the invoice is BOLT11 and has no amount
    ///
    /// This is for internal use only. External consumers should use `amount_sats()`.
    pub(crate) fn amount_msats(&self) -> Result<u64, Error> {
        match self {
            Invoice::Bolt11(invoice) => invoice
                .amount_milli_satoshis()
                .ok_or_else(|| Error::InvoiceWithoutAmount(invoice.to_string())),
            Invoice::Bolt12(invoice) => Ok(invoice.amount_msats()),
        }
    }

    /// Get the amount in whole satoshis from the invoice
    ///
    /// Returns an error if:
    /// - The invoice is BOLT11 and has no amount
    /// - The amount is not a whole number of satoshis
    pub fn amount_sats(&self) -> Result<u64, Error> {
        let msats = self.amount_msats()?;
        if msats % 1000 == 0 {
            Ok(msats / 1000)
        } else {
            Err(Error::Generic(
                "Invoice amount is not a whole sat".to_string(),
            ))
        }
    }

    /// Get the BOLT11 invoice if this is a BOLT11 variant
    pub fn bolt11(&self) -> Option<&Bolt11Invoice> {
        match self {
            Invoice::Bolt11(invoice) => Some(invoice.as_ref()),
            Invoice::Bolt12(_) => None,
        }
    }

    /// Get the BOLT12 invoice if this is a BOLT12 variant
    pub fn bolt12(&self) -> Option<&Bolt12Invoice> {
        match self {
            Invoice::Bolt11(_) => None,
            Invoice::Bolt12(invoice) => Some(invoice.as_ref()),
        }
    }

    /// Check if this is a BOLT11 invoice
    pub fn is_bolt11(&self) -> bool {
        matches!(self, Invoice::Bolt11(_))
    }

    /// Check if this is a BOLT12 invoice
    pub fn is_bolt12(&self) -> bool {
        matches!(self, Invoice::Bolt12(_))
    }
}

impl From<Bolt11Invoice> for Invoice {
    fn from(invoice: Bolt11Invoice) -> Self {
        Invoice::Bolt11(Box::new(invoice))
    }
}

impl From<Bolt12Invoice> for Invoice {
    fn from(invoice: Bolt12Invoice) -> Self {
        Invoice::Bolt12(Box::new(invoice))
    }
}

impl fmt::Display for Invoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invoice::Bolt11(invoice) => write!(f, "{}", invoice),
            Invoice::Bolt12(invoice) => write!(f, "{}", crate::display_bolt12_invoice(invoice)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_bolt11_invoice() {
        let invoice_str = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let bolt11 = Bolt11Invoice::from_str(invoice_str).unwrap();
        let invoice = Invoice::from(bolt11.clone());

        assert!(invoice.is_bolt11());
        assert!(!invoice.is_bolt12());
        assert!(invoice.bolt11().is_some());
        assert!(invoice.bolt12().is_none());
        assert_eq!(invoice.to_string(), invoice_str);
        assert_eq!(invoice.amount_msats().unwrap(), 2323000);
        assert_eq!(invoice.amount_sats().unwrap(), 2323);
    }

    #[test]
    fn test_bolt12_invoice() {
        let invoice_str = "lni1qqgwwn892vxqk9fsgul2fgzxyj5wk93pqtqft5rf2w8ed0c5chus7mqg2x7lx49qajrq8x3yhuu2w0msttwzc5srqxr2q4qqtqss80rn9yedw8hsef9w2lwa83zsfxglnhaen4kl272wrv4uccukswxm5zvq9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpvqws6qvd2q863an980srs7dpnt6qpqzlxrdkds6l8zz33enxmr42ujqgzfyq6zkdznkzf5m4u7ran24078mtlcdnaltufm4znls5gkq9lyhvqqvhwq0uy4rzc77s7d8gfx4hxemjql7gfcd7l97c3m76vtqnqmkg3eafm2msn4jj864haz42dc6r8r47gt64zrsqqqqqqqqqqqqqqzgqqqqqqqqqqqqqayjedltzjqqqqqq9yq35mrksp4qst37he8z5zvgq948434andxfzlfru53mfvvaycmed6ynt67qyg3xa2qvqcdg9wqvpqqq9syypvp9wsd9fcl94lznzljrmvppgmmu655rkgvqu6yjln3felwpddct8sgrt30e0uynvhy5ydaktehuwctyzkd05wgw4zqn0ayx4d9yndcfhd4ygpjceygz9629n4qm0zn7xa5k8e8xaphu280n4v2y3dzc2etywv";
        let bolt12 = crate::parse_bolt12_invoice(invoice_str).unwrap();
        let invoice = Invoice::from(bolt12);

        assert!(!invoice.is_bolt11());
        assert!(invoice.is_bolt12());
        assert!(invoice.bolt11().is_none());
        assert!(invoice.bolt12().is_some());
        assert_eq!(invoice.to_string(), invoice_str);
        assert!(invoice.amount_msats().unwrap() > 0);
    }
}
