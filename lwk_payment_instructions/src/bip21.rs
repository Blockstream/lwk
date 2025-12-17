use std::{convert::Infallible, fmt::Display, str::FromStr};

use bip21_crate::de::{DeserializationError, DeserializationState, DeserializeParams, ParamKind};
use elements::bitcoin::address::NetworkUnchecked;
use lightning::offers::offer::Offer;
use lightning_invoice::Bolt11Invoice;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bip21(String);

impl Bip21 {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn parsed(&self) -> bip21_crate::Uri<'_, NetworkUnchecked, Extras> {
        // Safe to unwrap because we validated the string in from_str
        bip21_crate::Uri::from_str(&self.0).unwrap()
    }

    /// Returns the Bitcoin address from the BIP21 URI
    pub fn address(&self) -> elements::bitcoin::Address<NetworkUnchecked> {
        self.parsed().address.clone()
    }

    pub fn amount(&self) -> Option<u64> {
        self.parsed().amount.map(|a| a.to_sat())
    }

    pub fn label(&self) -> Option<String> {
        self.parsed().label.and_then(|l| l.try_into().ok())
    }

    pub fn message(&self) -> Option<String> {
        self.parsed().message.and_then(|m| m.try_into().ok())
    }

    pub fn lightning(&self) -> Option<Bolt11Invoice> {
        self.parsed().extras.lightning
    }

    pub fn offer(&self) -> Option<Offer> {
        self.parsed().extras.offer
    }

    pub fn pj(&self) -> Option<url::Url> {
        self.parsed().extras.pj
    }

    pub fn pjos(&self) -> bool {
        self.parsed().extras.pjos
    }
}

impl PartialEq<str> for Bip21 {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl FromStr for Bip21 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let _bip21: bip21_crate::Uri<'_, NetworkUnchecked, Extras> =
            bip21_crate::Uri::from_str(s).map_err(|e| e.to_string())?;
        Ok(Self(s.to_string()))
    }
}

impl Display for Bip21 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Extra BIP21 parameters including lightning invoice and BOLT12 offer
#[derive(Clone, Debug, Default)]
struct Extras {
    lightning: Option<Bolt11Invoice>,
    offer: Option<Offer>,
    pj: Option<url::Url>,
    /// Payjoin output substitution, defaults to true if absent
    pjos: bool,
}

impl DeserializationError for Extras {
    type Error = Infallible;
}

impl DeserializeParams<'_> for Extras {
    type DeserializationState = ExtrasState;
}

struct ExtrasState {
    lightning: Option<Bolt11Invoice>,
    offer: Option<Offer>,
    pj: Option<url::Url>,
    /// Defaults to true if absent
    pjos: bool,
}

impl Default for ExtrasState {
    fn default() -> Self {
        Self {
            lightning: None,
            offer: None,
            pj: None,
            pjos: true,
        }
    }
}

impl DeserializationState<'_> for ExtrasState {
    type Value = Extras;

    fn is_param_known(&self, key: &str) -> bool {
        key.eq_ignore_ascii_case("lightning")
            || key.eq_ignore_ascii_case("lno")
            || key.eq_ignore_ascii_case("pj")
            || key.eq_ignore_ascii_case("pjos")
    }

    fn deserialize_temp(
        &mut self,
        key: &str,
        value: bip21_crate::Param<'_>,
    ) -> Result<ParamKind, Infallible> {
        if key.eq_ignore_ascii_case("lightning") {
            if let Ok(s) = String::try_from(value) {
                self.lightning = Bolt11Invoice::from_str(&s).ok();
            }
            Ok(ParamKind::Known)
        } else if key.eq_ignore_ascii_case("lno") {
            if let Ok(s) = String::try_from(value) {
                self.offer = Offer::from_str(&s).ok();
            }
            Ok(ParamKind::Known)
        } else if key.eq_ignore_ascii_case("pj") {
            if let Ok(s) = String::try_from(value) {
                self.pj = url::Url::from_str(&s).ok();
            }
            Ok(ParamKind::Known)
        } else if key.eq_ignore_ascii_case("pjos") {
            if let Ok(s) = String::try_from(value) {
                if s == "0" {
                    self.pjos = false;
                }
            }
            Ok(ParamKind::Known)
        } else {
            Ok(ParamKind::Unknown)
        }
    }

    fn finalize(self) -> Result<Extras, Infallible> {
        Ok(Extras {
            lightning: self.lightning,
            offer: self.offer,
            pj: self.pj,
            pjos: self.pjos,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bip21_from_str() {
        let bip21 = Bip21::from_str("bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=0.0001&label=Test&message=Hello%2C%20world!").unwrap();
        assert_eq!(bip21.amount(), Some(10000));
        assert_eq!(bip21.label(), Some("Test".to_string()));
        assert_eq!(bip21.message(), Some("Hello, world!".to_string()));

        let lightning_invoice = "LNBC10U1P3PJ257PP5YZTKWJCZ5FTL5LAXKAV23ZMZEKAW37ZK6KMV80PK4XAEV5QHTZ7QDPDWD3XGER9WD5KWM36YPRX7U3QD36KUCMGYP282ETNV3SHJCQZPGXQYZ5VQSP5USYC4LK9CHSFP53KVCNVQ456GANH60D89REYKDNGSMTJ6YW3NHVQ9QYYSSQJCEWM5CJWZ4A6RFJX77C490YCED6PEMK0UPKXHY89CMM7SCT66K8GNEANWYKZGDRWRFJE69H9U5U0W57RRCSYSAS7GADWMZXC8C6T0SPJAZUP6";
        let unified_bolt11 = format!("bitcoin:BC1QYLH3U67J673H6Y6ALV70M0PL2YZ53TZHVXGG7U?amount=0.00001&label=sbddesign%3A%20For%20lunch%20Tuesday&message=For%20lunch%20Tuesday&lightning={lightning_invoice}");
        let bip21 = Bip21::from_str(&unified_bolt11).unwrap();
        assert_eq!(bip21.amount(), Some(1000)); // 0.00001 BTC = 1000 sats
        assert_eq!(
            bip21.label(),
            Some("sbddesign: For lunch Tuesday".to_string())
        );
        assert_eq!(bip21.message(), Some("For lunch Tuesday".to_string()));
        assert_eq!(
            bip21.lightning(),
            Some(Bolt11Invoice::from_str(lightning_invoice).unwrap())
        );

        let bolt12 = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";
        let unified_bolt12 = format!("bitcoin:BC1QYLH3U67J673H6Y6ALV70M0PL2YZ53TZHVXGG7U?amount=0.00001&label=sbddesign%3A%20For%20lunch%20Tuesday&message=For%20lunch%20Tuesday&lno={bolt12}");
        let bip21 = Bip21::from_str(&unified_bolt12).unwrap();
        assert_eq!(bip21.amount(), Some(1000)); // 0.00001 BTC = 1000 sats
        assert_eq!(
            bip21.label(),
            Some("sbddesign: For lunch Tuesday".to_string())
        );
        assert_eq!(bip21.message(), Some("For lunch Tuesday".to_string()));
        assert_eq!(bip21.offer(), Some(Offer::from_str(bolt12).unwrap()));
    }

    #[test]
    fn test_deserialize_pjos() {
        // pjos=0 should disable output substitution
        let uri = "bitcoin:12c6DSiU4Rq3P4ZxziKxzrL5LmMBrzjrJX?pj=https://example.com&pjos=0";
        let bip21 = Bip21::from_str(uri).unwrap();
        assert!(!bip21.pjos());
        assert_eq!(
            bip21.pj(),
            Some(url::Url::from_str("https://example.com").unwrap())
        );

        // pjos=1 should allow output substitution
        let uri = "bitcoin:12c6DSiU4Rq3P4ZxziKxzrL5LmMBrzjrJX?pj=https://example.com&pjos=1";
        let bip21 = Bip21::from_str(uri).unwrap();
        assert!(bip21.pjos());
        assert_eq!(
            bip21.pj(),
            Some(url::Url::from_str("https://example.com").unwrap())
        );

        // Elided pjos should allow output substitution (default to true)
        let uri = "bitcoin:12c6DSiU4Rq3P4ZxziKxzrL5LmMBrzjrJX?pj=https://example.com";
        let bip21 = Bip21::from_str(uri).unwrap();
        assert!(bip21.pjos());
        assert_eq!(
            bip21.pj(),
            Some(url::Url::from_str("https://example.com").unwrap())
        );
    }

    #[test]
    fn test_payjoin_with_amount() {
        let uri =
            "bitcoin:12c6DSiU4Rq3P4ZxziKxzrL5LmMBrzjrJX?amount=0.01&pjos=0&pj=https://example.com";
        let bip21 = Bip21::from_str(uri).unwrap();
        assert_eq!(bip21.amount(), Some(1_000_000)); // 0.01 BTC = 1_000_000 sats
        assert!(!bip21.pjos());
        assert_eq!(
            bip21.pj(),
            Some(url::Url::from_str("https://example.com").unwrap())
        );
    }

    #[test]
    fn test_payjoin_shuffled_params() {
        // pj before pjos
        let uri = "bitcoin:12c6DSiU4Rq3P4ZxziKxzrL5LmMBrzjrJX?pj=https://example.com&pjos=0";
        let bip21 = Bip21::from_str(uri).unwrap();
        assert!(!bip21.pjos());

        // amount between pj params
        let uri =
            "bitcoin:12c6DSiU4Rq3P4ZxziKxzrL5LmMBrzjrJX?pjos=0&amount=0.01&pj=https://example.com";
        let bip21 = Bip21::from_str(uri).unwrap();
        assert!(!bip21.pjos());
        assert_eq!(bip21.amount(), Some(1_000_000));
    }
}
