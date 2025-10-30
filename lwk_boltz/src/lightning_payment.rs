use std::{fmt::Display, str::FromStr};

use boltz_client::{lightning_invoice::ParseOrSemanticError, Bolt11Invoice};
use lightning::offers::{offer::Offer, parse::Bolt12ParseError};

#[derive(Debug)]
pub enum LightningPayment {
    Bolt11(Bolt11Invoice),
    Bolt12(Offer),
}

impl FromStr for LightningPayment {
    type Err = (ParseOrSemanticError, Bolt12ParseError);

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Bolt11Invoice::from_str(s) {
            Ok(invoice) => Ok(LightningPayment::Bolt11(invoice)),
            Err(e1) => match Offer::from_str(s) {
                Ok(offer) => Ok(LightningPayment::Bolt12(offer)),
                Err(e2) => Err((e1, e2)),
            },
        }
    }
}

impl Display for LightningPayment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LightningPayment::Bolt11(invoice) => write!(f, "{}", invoice),
            LightningPayment::Bolt12(offer) => write!(f, "{}", offer),
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
        let payment = LightningPayment::from_str(invoice).unwrap();
        assert!(matches!(payment, LightningPayment::Bolt11(_)));
        assert_eq!(payment.to_string(), invoice);
        let payment = LightningPayment::from_str(offer).unwrap();
        assert!(matches!(payment, LightningPayment::Bolt12(_)));
        assert_eq!(payment.to_string(), offer);
        let err = "not a valid invoice or offer";
        let err = LightningPayment::from_str(err).unwrap_err();
        assert!(matches!(err, (_, _)));
    }
}
