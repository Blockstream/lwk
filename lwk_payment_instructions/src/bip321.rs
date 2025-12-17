use std::{fmt::Display, str::FromStr};

use lightning::offers::offer::Offer;
use lightning_invoice::Bolt11Invoice;
use silentpayments::SilentPaymentAddress;

use crate::bip21::Bip21;

/// A mockup Bitcoin address used to inject into URIs without an address
/// This is a valid P2PKH mainnet address (Satoshi's genesis block address)
const MOCKUP_ADDRESS: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";

/// A parsed Bitcoin BIP321 URI with optional parameters.
///
/// BIP321 extends BIP21 by allowing URIs without a bitcoin address in the path,
/// as long as there is at least one payment instruction in the query parameters.
///
/// For example: `bitcoin:?ark=ark1qq...&amount=0.00000222`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bip321 {
    inner: Bip21,
    /// The original URI string
    original: String,
}

impl Bip321 {
    pub fn as_str(&self) -> &str {
        &self.original
    }

    pub fn amount(&self) -> Option<u64> {
        self.inner.amount()
    }

    pub fn label(&self) -> Option<String> {
        self.inner.label()
    }

    pub fn message(&self) -> Option<String> {
        self.inner.message()
    }

    pub fn lightning(&self) -> Option<Bolt11Invoice> {
        self.inner.lightning()
    }

    pub fn offer(&self) -> Option<Offer> {
        self.inner.offer()
    }

    pub fn payjoin(&self) -> Option<url::Url> {
        self.inner.payjoin()
    }

    pub fn payjoin_output_substitution(&self) -> bool {
        self.inner.payjoin_output_substitution()
    }

    pub fn silent_payment_address(&self) -> Option<SilentPaymentAddress> {
        self.inner.silent_payment_address()
    }

    /// Returns the ark address from the URI if present
    pub fn ark(&self) -> Option<String> {
        self.inner.ark()
    }
}

impl PartialEq<str> for Bip321 {
    fn eq(&self, other: &str) -> bool {
        self.original == other
    }
}

impl FromStr for Bip321 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // First try to parse as regular Bip21 (with address)
        if let Ok(bip21) = Bip21::from_str(s) {
            return Ok(Self {
                inner: bip21,
                original: s.to_string(),
            });
        }

        // Try to parse as URL first to validate structure
        let url = url::Url::from_str(s).map_err(|e| e.to_string())?;

        // Check that the scheme is "bitcoin" (case-insensitive)
        if !url.scheme().eq_ignore_ascii_case("bitcoin") {
            return Err(format!("Invalid scheme: {}", url.scheme()));
        }

        // BIP321: if no address, there must be at least one query parameter
        if url.query().is_none() || url.query().map(|q| q.is_empty()).unwrap_or(true) {
            return Err("BIP321 URI without address must have query parameters".to_string());
        }

        // Build a new URI with the mockup address
        let modified_uri = format!(
            "bitcoin:{MOCKUP_ADDRESS}?{}",
            url.query().expect("just checked that it's not empty")
        );
        let inner = Bip21::from_str(&modified_uri)?;

        let known_payment = inner.ark().is_some()
            || inner.silent_payment_address().is_some()
            || inner.lightning().is_some()
            || inner.offer().is_some();
        if !known_payment {
            return Err("BIP321 URI without address must have another payment method".to_string());
        }

        Ok(Self {
            inner,
            original: s.to_string(),
        })
    }
}

impl Display for Bip321 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.original)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bip321_without_address() {
        // URI with ark parameter and no address
        let uri =
            "bitcoin:?ark=ark1qq4hfssprtcgnjzf8qlw2f78yvjau5kldfugg29k34y7j96q2w4t567uy9ukgfl2ntulzvlzj7swsprfs4wy4h47m7z48khygt7qsyazckttpz&amount=0.00000222";
        let bip321 = Bip321::from_str(uri).unwrap();
        assert_eq!(bip321.amount(), Some(222)); // 0.00000222 BTC = 222 sats
        assert_eq!(
            bip321.ark(),
            Some("ark1qq4hfssprtcgnjzf8qlw2f78yvjau5kldfugg29k34y7j96q2w4t567uy9ukgfl2ntulzvlzj7swsprfs4wy4h47m7z48khygt7qsyazckttpz".to_string())
        );
        assert_eq!(bip321.as_str(), uri);
    }

    #[test]
    fn test_bip321_with_lightning_no_address() {
        // URI with lightning parameter and no address (from BIP321 examples)
        let uri = "bitcoin:?lightning=lnbc420bogusinvoice";
        // This will fail to parse lightning invoice but should still parse the URI
        let bip321_err = Bip321::from_str(uri).unwrap_err();
        assert_eq!(
            bip321_err,
            "BIP321 URI without address must have another payment method"
        );
    }

    #[test]
    fn test_bip321_with_address() {
        // Regular BIP21 URI should also work
        let uri = format!("bitcoin:{MOCKUP_ADDRESS}?amount=0.001");
        let bip321 = Bip321::from_str(&uri).unwrap();
        assert_eq!(bip321.amount(), Some(100_000)); // 0.001 BTC = 100_000 sats
    }

    #[test]
    fn test_bip321_no_params_fails() {
        // URI without address and without query params should fail
        let uri = "bitcoin:";
        let result = Bip321::from_str(uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_bip321_empty_query_fails() {
        // URI without address and with empty query should fail
        let uri = "bitcoin:?";
        let result = Bip321::from_str(uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_bip321_uppercase_scheme() {
        // Uppercase scheme should work (per BIP321)
        let uri = "BITCOIN:?ark=somearkaddress&amount=0.001";
        let bip321 = Bip321::from_str(uri).unwrap();
        assert_eq!(bip321.amount(), Some(100_000));
        assert_eq!(bip321.ark(), Some("somearkaddress".to_string()));
    }

    #[test]
    fn test_bip321_display() {
        let uri = "bitcoin:?ark=somearkaddress&amount=0.001";
        let bip321 = Bip321::from_str(uri).unwrap();
        assert_eq!(format!("{bip321}"), uri);
    }

    #[test]
    fn test_bip321_multiple_params() {
        let uri =
            "bitcoin:?ark=ark1testaddr&amount=0.00001&label=Test%20Payment&message=Hello%20World";
        let bip321 = Bip321::from_str(uri).unwrap();
        assert_eq!(bip321.amount(), Some(1_000)); // 0.00001 BTC = 1000 sats
        assert_eq!(bip321.ark(), Some("ark1testaddr".to_string()));
        assert_eq!(bip321.label(), Some("Test Payment".to_string()));
        assert_eq!(bip321.message(), Some("Hello World".to_string()));
    }

    #[test]
    fn test_bip321_silent_payment_no_address() {
        // Silent payment address with no fallback (per BIP321 examples)
        let sp_address = "sp1qqgste7k9hx0qftg6qmwlkqtwuy6cycyavzmzj85c6qdfhjdpdjtdgqjuexzk6murw56suy3e0rd2cgqvycxttddwsvgxe2usfpxumr70xc9pkqwv";
        let uri = format!("bitcoin:?sp={sp_address}");
        let bip321 = Bip321::from_str(&uri).unwrap();
        let parsed_sp = bip321.silent_payment_address();
        assert!(parsed_sp.is_some());
        assert_eq!(parsed_sp.unwrap().to_string(), sp_address);
    }
}
