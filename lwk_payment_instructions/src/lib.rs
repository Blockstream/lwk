use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
};

use bitcoin_payment_instructions::{
    dns_resolver::DNSHrnResolver, hrn_resolution::HrnResolver, PaymentInstructions, PaymentMethod,
    PossiblyResolvedPaymentMethod,
};
use elements::{
    bitcoin::{self, address::NetworkUnchecked},
    AddressParams, AssetId,
};
use lightning::offers::offer::Offer;
use lightning_invoice::Bolt11Invoice;

mod bip21;
mod bip321;
mod error;
mod lnurl;
use ::lnurl::lnurl::LnUrl;
pub use bip21::Bip21;
pub use bip321::Bip321;
pub use error::Error;
pub use lnurl::{LnUrlIdentifier, LnUrlPayResponse};

use crate::lnurl::LnUrlInvoiceResponse;

#[allow(dead_code)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentKind {
    BitcoinAddress,
    LiquidAddress,
    LightningInvoice,
    LightningOffer,
    LnUrl,
    Bip353,
    Bip21,
    Bip321,

    /// Liquid BIP21 URI, follows rules defined in https://github.com/ElementsProject/elements/issues/805
    LiquidBip21,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct LiquidBip21 {
    pub address: elements::Address,
    pub asset: AssetId,

    /// The amount in satoshis or units of the asset (optional)
    pub satoshi: Option<u64>,
}

#[allow(dead_code)]
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum Payment {
    BitcoinAddress(bitcoin::Address<bitcoin::address::NetworkUnchecked>), // just the address, or bitcoin:<address>
    LiquidAddress(elements::Address), // just the address, or liquidnetwork:<address> or liquidtestnet:<address>
    LightningInvoice(Bolt11Invoice),  // just the invoice or lightning:<invoice>
    LightningOffer(Box<Offer>),       // just the bolt12 or lightning:<bolt12>
    LnUrlCat(LnUrlIdentifier), // just lnurl or lightning:<lnurl> or lightning:<lud16> or lnurlp://<url>
    Bip353(String),            // ₿matt@mattcorallo.com
    Bip21(Bip21),              // bitcoin:
    Bip321(Bip321),            // bitcoin: uri without an address but with a payment method
    LiquidBip21(LiquidBip21),  // liquidnetwork: liquidtestnet:
}

impl Payment {
    pub fn kind(&self) -> PaymentKind {
        match self {
            Payment::BitcoinAddress(_) => PaymentKind::BitcoinAddress,
            Payment::LiquidAddress(_) => PaymentKind::LiquidAddress,
            Payment::LightningInvoice(_) => PaymentKind::LightningInvoice,
            Payment::LightningOffer(_) => PaymentKind::LightningOffer,
            Payment::LnUrlCat(_) => PaymentKind::LnUrl,
            Payment::Bip353(_) => PaymentKind::Bip353,
            Payment::Bip21(_) => PaymentKind::Bip21,
            Payment::LiquidBip21(_) => PaymentKind::LiquidBip21,
            Payment::Bip321(_) => PaymentKind::Bip321,
        }
    }

    pub fn bitcoin_address(&self) -> Option<&bitcoin::Address<NetworkUnchecked>> {
        match self {
            Payment::BitcoinAddress(addr) => Some(addr),
            _ => None,
        }
    }

    pub fn liquid_address(&self) -> Option<&elements::Address> {
        match self {
            Payment::LiquidAddress(addr) => Some(addr),
            _ => None,
        }
    }

    pub fn lightning_invoice(&self) -> Option<&Bolt11Invoice> {
        match self {
            Payment::LightningInvoice(invoice) => Some(invoice),
            _ => None,
        }
    }

    pub fn lightning_offer(&self) -> Option<&Offer> {
        match self {
            Payment::LightningOffer(offer) => Some(offer),
            _ => None,
        }
    }

    pub fn lnurl(&self) -> Option<&LnUrlIdentifier> {
        match self {
            Payment::LnUrlCat(lnurl) => Some(lnurl),
            _ => None,
        }
    }

    pub fn bip353(&self) -> Option<&str> {
        match self {
            Payment::Bip353(s) => Some(s),
            _ => None,
        }
    }

    pub fn bip21(&self) -> Option<&Bip21> {
        match self {
            Payment::Bip21(bip21) => Some(bip21),
            _ => None,
        }
    }

    pub fn bip321(&self) -> Option<&Bip321> {
        match self {
            Payment::Bip321(bip321) => Some(bip321),
            _ => None,
        }
    }

    pub fn liquid_bip21(&self) -> Option<&LiquidBip21> {
        match self {
            Payment::LiquidBip21(bip21) => Some(bip21),
            _ => None,
        }
    }

    /// Resolves a LNURL into its metadata (first step of LNURL-pay).
    pub async fn resolve_lnurl_info(&self) -> Result<LnUrlPayResponse, Error> {
        let lnurl = self
            .lnurl()
            .ok_or(Error::ExpectedKind(PaymentKind::LnUrl))?;

        let url_str = lnurl.resolve_url()?;

        let client = reqwest::Client::new();
        let resp = client
            .get(url_str)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch LNURL info: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("LNURL server returned error: {}", resp.status()).into());
        }

        let info: LnUrlPayResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse LNURL info: {e}"))?;

        if info.tag != "payRequest" {
            return Err(format!("Unsupported LNURL tag: {}", info.tag).into());
        }

        Ok(info)
    }

    /// Fetches a Bolt11 invoice from a LNURL callback (second step of LNURL-pay).
    pub async fn fetch_lnurl_invoice(
        info: &LnUrlPayResponse,
        amount_sats: u64,
    ) -> Result<Self, Error> {
        let amount_msat = amount_sats
            .checked_mul(1000)
            .ok_or_else(|| "Amount overflow".to_string())?;
        if amount_msat < info.min_sendable || amount_msat > info.max_sendable {
            return Err(format!(
                "Amount {} sats ({} msat) is out of range [{} msat, {} msat]",
                amount_sats, amount_msat, info.min_sendable, info.max_sendable
            )
            .into());
        }

        let mut url = url::Url::parse(&info.callback).map_err(|e| e.to_string())?;
        url.query_pairs_mut()
            .append_pair("amount", &amount_msat.to_string());

        let client = reqwest::Client::new();
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch LNURL invoice: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("LNURL callback returned error: {}", resp.status()).into());
        }

        let res: LnUrlInvoiceResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse LNURL invoice response: {e}"))?;

        if let Some(status) = res.status {
            if status == "ERROR" {
                return Err(format!(
                    "LNURL error: {}",
                    res.reason.unwrap_or_else(|| "Unknown error".to_string())
                )
                .into());
            }
        }

        let invoice = Bolt11Invoice::from_str(&res.pr).map_err(|e| e.to_string())?;
        Ok(Payment::LightningInvoice(invoice))
    }

    /// Resolves a BIP353 payment instruction into a lightning offer.
    pub async fn resolve_bip353(&self) -> Result<Self, Error> {
        let bip353 = self
            .bip353()
            .ok_or(Error::ExpectedKind(PaymentKind::Bip353))?;

        // we use google dns server to resolve
        let resolver = DNSHrnResolver(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53));
        // we may want to try HTTPHrnResolver when DNSHrnResolver fails
        let offer = resolve_bip353_with_resolver(bip353, &resolver).await?;
        Ok(Payment::LightningOffer(Box::new(offer)))
    }
}

async fn resolve_bip353_with_resolver<R: HrnResolver>(
    bip353: &str,
    resolver: &R,
) -> Result<Offer, Error> {
    let instructions =
        PaymentInstructions::parse(bip353, bitcoin::Network::Bitcoin, resolver, true)
            .await
            .map_err(|e| format!("{e:?}"))?;

    match instructions {
        PaymentInstructions::FixedAmount(fixed) => fixed
            .methods()
            .iter()
            .find_map(|method| match method {
                PaymentMethod::LightningBolt12(offer) => {
                    Some(Offer::from_str(&offer.to_string()).map_err(|e| format!("{e:?}")))
                }
                _ => None,
            })
            .transpose()?
            .ok_or_else(|| "BIP353 did not resolve to a lightning offer".into()),
        PaymentInstructions::ConfigurableAmount(configurable) => configurable
            .methods()
            .find_map(|method| match method {
                PossiblyResolvedPaymentMethod::Resolved(PaymentMethod::LightningBolt12(offer)) => {
                    Some(Offer::from_str(&offer.to_string()).map_err(|e| format!("{e:?}")))
                }
                _ => None,
            })
            .transpose()?
            .ok_or_else(|| "BIP353 did not resolve to a lightning offer".into()),
    }
}

enum Schema {
    Bitcoin,
    LiquidNetwork,
    LiquidTestnet,
    Lightning,
    LnUrlP,
}

impl FromStr for Schema {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "bitcoin" => Ok(Schema::Bitcoin),
            "liquidnetwork" => Ok(Schema::LiquidNetwork),
            "liquidtestnet" => Ok(Schema::LiquidTestnet),
            "lightning" => Ok(Schema::Lightning),
            "lnurlp" => Ok(Schema::LnUrlP),
            "BITCOIN" => Ok(Schema::Bitcoin),
            "LIQUIDNETWORK" => Ok(Schema::LiquidNetwork),
            "LIQUIDTESTNET" => Ok(Schema::LiquidTestnet),
            "LIGHTNING" => Ok(Schema::Lightning),
            "LNURLP" => Ok(Schema::LnUrlP),
            _ => Err(format!("Invalid schema: {s}").into()),
        }
    }
}

impl FromStr for Payment {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((prefix, rest)) => {
                let schema = Schema::from_str(prefix)?;
                let cat = parse_no_schema(rest);
                parse_with_schema(schema, cat, s)
            }
            None => parse_no_schema(s),
        }
    }
}

fn parse_with_schema(
    schema: Schema,
    cat: Result<Payment, Error>,
    s: &str,
) -> Result<Payment, Error> {
    use Payment::*;
    use Schema::*;
    match (schema, cat) {
        (Bitcoin, Ok(cat @ BitcoinAddress(_))) => Ok(cat),
        (Bitcoin, Err(_)) => match bip21::Bip21::from_str(s) {
            Ok(bip21) => Ok(Bip21(bip21)),
            Err(_) => match bip321::Bip321::from_str(s) {
                Ok(bip321) => Ok(Bip321(bip321)),
                Err(_) => Err(format!("Invalid bip21 or bip321 URI: {s}").into()),
            },
        },

        (LiquidNetwork, Ok(ref cat @ LiquidAddress(ref a))) => {
            if a.params == &AddressParams::LIQUID {
                Ok(cat.clone())
            } else {
                Err(Error::WrongLiquidNetwork {
                    expected_mainnet: true,
                })
            }
        }
        (LiquidNetwork, Err(_)) => parse_liquid_bip21(s, true),
        (LiquidTestnet, Ok(ref cat @ LiquidAddress(ref a))) => {
            if a.params != &AddressParams::LIQUID {
                Ok(cat.clone())
            } else {
                Err(Error::WrongLiquidNetwork {
                    expected_mainnet: false,
                })
            }
        }
        (LiquidTestnet, Err(_)) => parse_liquid_bip21(s, false),
        (Lightning, Ok(cat @ LightningInvoice(_))) => Ok(cat),
        (Lightning, Ok(cat @ LightningOffer(_))) => Ok(cat),
        (Lightning, Ok(cat @ LnUrlCat(_))) => Ok(cat),
        (Lightning, Err(_)) => {
            // lightning:<email> can be an lnurl
            let rest = &s[10..];
            if is_email(rest) {
                Ok(LnUrlCat(LnUrlIdentifier::Lud16(rest.to_string())))
            } else {
                Err(format!("Invalid lightning schema: {s}").into())
            }
        }
        (LnUrlP, _) => {
            // lnurlp://<url> can be an lnurl
            url::Url::from_str(s).map_err(|e| e.to_string())?;
            let lnurl = LnUrl::from_url(s.to_string());
            Ok(LnUrlCat(LnUrlIdentifier::LnUrl(lnurl)))
        }
        _ => Err(format!("Invalid schema: {s}").into()),
    }
}

fn parse_liquid_bip21(s: &str, is_mainnet: bool) -> Result<Payment, Error> {
    let url = url::Url::from_str(s).map_err(|e| e.to_string())?;

    let address_str = url.path();
    let address = elements::Address::from_str(address_str).map_err(|e| e.to_string())?;

    let is_liquid_mainnet = address.params == &AddressParams::LIQUID;
    if is_mainnet && !is_liquid_mainnet {
        return Err(Error::WrongLiquidNetwork {
            expected_mainnet: true,
        });
    }
    if !is_mainnet && is_liquid_mainnet {
        return Err(Error::WrongLiquidNetwork {
            expected_mainnet: false,
        });
    }

    let asset_str = url
        .query_pairs()
        .find(|(key, _)| key == "assetid")
        .map(|(_, value)| value)
        .ok_or_else(|| "Invalid payment request: assetID needs to be specified".to_string())?;
    let asset = AssetId::from_str(&asset_str).map_err(|e| e.to_string())?;

    // BIP21 amounts are in BTC (decimal), convert to satoshis (optional)
    let amount = url
        .query_pairs()
        .find(|(key, _)| key == "amount")
        .map(|(_, value)| {
            bitcoin::Amount::from_str_in(&value, bitcoin::Denomination::Bitcoin).map(|a| a.to_sat())
        })
        .transpose()
        .map_err(|e| e.to_string())?;

    Ok(Payment::LiquidBip21(LiquidBip21 {
        address,
        asset,
        satoshi: amount,
    }))
}

fn parse_no_schema(s: &str) -> Result<Payment, Error> {
    if let Ok(bitcoin_address) = bitcoin::Address::from_str(s) {
        return Ok(Payment::BitcoinAddress(bitcoin_address));
    }
    if let Ok(liquid_address) = elements::Address::from_str(s) {
        return Ok(Payment::LiquidAddress(liquid_address));
    }
    if let Ok(lightning_invoice) = Bolt11Invoice::from_str(s) {
        return Ok(Payment::LightningInvoice(lightning_invoice));
    }
    if let Ok(lightning_offer) = Offer::from_str(s) {
        return Ok(Payment::LightningOffer(Box::new(lightning_offer)));
    }
    if let Ok(lnurl) = LnUrl::from_str(s) {
        return Ok(Payment::LnUrlCat(LnUrlIdentifier::LnUrl(lnurl)));
    }
    if s.starts_with("₿") {
        let rest = s.chars().skip(1).collect::<String>();
        if is_email(&rest) {
            return Ok(Payment::Bip353(rest));
        }
    }
    if is_email(s) {
        return Ok(Payment::LnUrlCat(LnUrlIdentifier::Lud16(s.to_string())));
    }
    Err(format!("Invalid payment category: {s}").into())
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.is_empty() {
        return false;
    }

    if IpAddr::from_str(domain).is_ok() || SocketAddr::from_str(domain).is_ok() {
        return cfg!(debug_assertions);
    }

    let is_regular_domain =
        domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.');
    let is_localhost = domain == "localhost";

    is_regular_domain || (cfg!(debug_assertions) && is_localhost)
}

#[cfg(test)]
mod tests {
    use ::lnurl::lnurl::LnUrl;
    use bitcoin_payment_instructions::{
        amount::Amount,
        hrn_resolution::{
            HrnResolution, HrnResolutionFuture, HumanReadableName, LNURLResolutionFuture,
        },
    };

    use super::*;

    struct TestHrnResolver {
        result: &'static str,
    }

    impl HrnResolver for TestHrnResolver {
        fn resolve_hrn<'a>(&'a self, _hrn: &'a HumanReadableName) -> HrnResolutionFuture<'a> {
            Box::pin(async move {
                Ok(HrnResolution::DNSSEC {
                    proof: None,
                    result: self.result.to_string(),
                })
            })
        }

        fn resolve_lnurl<'a>(&'a self, _lnurl: &'a str) -> HrnResolutionFuture<'a> {
            Box::pin(async { Err("LNURL resolution not supported") })
        }

        fn resolve_lnurl_to_invoice(
            &self,
            _: String,
            _: Amount,
            _: [u8; 32],
        ) -> LNURLResolutionFuture<'_> {
            Box::pin(async { Err("LNURL resolution not supported") })
        }
    }

    #[test]
    fn test_parse_with_schema_fails() {
        let payment_category = Payment::from_str("bitcoin:invalid_address").unwrap_err();
        assert_eq!(
            payment_category,
            "Invalid bip21 or bip321 URI: bitcoin:invalid_address".into()
        );

        // mixed case schema are not supported
        let payment_category =
            Payment::from_str("BITcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap_err();
        assert_eq!(payment_category, "Invalid schema: BITcoin".into());

        // valid mainnet address with testnet schema
        let payment_category = Payment::from_str("liquidtestnet:lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0").unwrap_err();
        assert_eq!(
            payment_category,
            Error::WrongLiquidNetwork {
                expected_mainnet: false
            }
        );

        // valid testnet address with mainnet schema
        let payment_category = Payment::from_str("liquidnetwork:tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m").unwrap_err();
        assert_eq!(
            payment_category,
            Error::WrongLiquidNetwork {
                expected_mainnet: true
            }
        );

        // valid testnet address with testnet schema
        let err = Payment::from_str("liquidtestnet:VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag?amount=10&assetid=ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2").unwrap_err();
        assert_eq!(
            err,
            Error::WrongLiquidNetwork {
                expected_mainnet: false
            }
        );
    }

    #[test]
    fn test_parse_with_schema() {
        let bitcoin_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let payment_category = Payment::from_str(&format!("bitcoin:{bitcoin_address}")).unwrap();
        let expected =
            bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(bitcoin_address)
                .unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::BitcoinAddress);
        assert_eq!(payment_category.bitcoin_address(), Some(&expected));
        assert!(payment_category.liquid_address().is_none());
        assert!(matches!(
            payment_category,
            Payment::BitcoinAddress(addr) if addr == expected
        ));
        let payment_category = Payment::from_str(&format!("BITCOIN:{bitcoin_address}")).unwrap();
        let expected =
            bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(bitcoin_address)
                .unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::BitcoinAddress);
        assert_eq!(payment_category.bitcoin_address(), Some(&expected));
        assert!(matches!(
            payment_category,
            Payment::BitcoinAddress(addr) if addr == expected
        ));

        let liquid_address = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let payment_category =
            Payment::from_str(&format!("liquidnetwork:{liquid_address}")).unwrap();
        let expected = elements::Address::from_str(liquid_address).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LiquidAddress);
        assert_eq!(payment_category.liquid_address(), Some(&expected));
        assert!(payment_category.bitcoin_address().is_none());
        assert!(matches!(
            payment_category,
            Payment::LiquidAddress(addr) if addr == expected
        ));

        let lightning_invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let payment_category =
            Payment::from_str(&format!("lightning:{lightning_invoice}")).unwrap();
        let expected = Bolt11Invoice::from_str(lightning_invoice).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LightningInvoice);
        assert_eq!(payment_category.lightning_invoice(), Some(&expected));
        assert!(payment_category.lightning_offer().is_none());
        assert!(matches!(
            payment_category,
            Payment::LightningInvoice(invoice) if invoice == expected
        ));

        let bolt12 = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";
        let payment_category = Payment::from_str(&format!("lightning:{bolt12}")).unwrap();
        let expected = Offer::from_str(bolt12).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LightningOffer);
        assert_eq!(payment_category.lightning_offer(), Some(&expected));
        assert!(payment_category.lightning_invoice().is_none());
        assert!(matches!(
            payment_category,
            Payment::LightningOffer(offer) if *offer == expected
        ));

        let bip21 = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50";
        let payment_category = Payment::from_str(bip21).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::Bip21);
        assert_eq!(payment_category.bip21().map(|b| b.as_str()), Some(bip21));
        assert!(payment_category.bitcoin_address().is_none());
        assert!(matches!(
            payment_category,
            Payment::Bip21(ref uri) if uri == bip21
        ));

        let bip21_upper = "BITCOIN:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50";
        let payment_category = Payment::from_str(bip21_upper).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::Bip21);
        assert_eq!(
            payment_category.bip21().map(|b| b.as_str()),
            Some(bip21_upper)
        );
        assert!(matches!(
            payment_category,
            Payment::Bip21(ref uri) if uri == bip21_upper
        ));

        // BIP321 URI with ark parameter and no address
        let ark_addr = "ark1qq4hfssprtcgnjzf8qlw2f78yvjau5kldfugg29k34y7j96q2w4t567uy9ukgfl2ntulzvlzj7swsprfs4wy4h47m7z48khygt7qsyazckttpz";
        let bip321 = format!("bitcoin:?ark={ark_addr}&amount=0.00000222");
        let payment_category = Payment::from_str(&bip321).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::Bip321);
        let bip321_ref = payment_category.bip321().unwrap();
        assert_eq!(bip321_ref.ark(), Some(ark_addr.to_string()));
        assert_eq!(bip321_ref.amount(), Some(222)); // 0.00000222 BTC = 222 sats
        assert!(payment_category.bip21().is_none());
        assert!(bip321_ref.address().is_none());

        let lnurl = "lnurl1dp68gurn8ghj7ctsdyhxwetewdjhytnxw4hxgtmvde6hymp0wpshj0mswfhk5etrw3ykg0f3xqcs2mcx97";
        let payment_category = Payment::from_str(&format!("lightning:{lnurl}")).unwrap();
        let expected = LnUrl::from_str(lnurl).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LnUrl);
        assert_eq!(payment_category.lnurl().unwrap().lnurl(), Some(&expected));
        assert!(payment_category.lnurl().unwrap().lud16().is_none());
        assert!(payment_category.lightning_invoice().is_none());
        assert!(matches!(
            payment_category,
            Payment::LnUrlCat(LnUrlIdentifier::LnUrl(lnurl)) if lnurl == expected
        ));

        let lnurlp = "lnurlp://geyser.fund/.well-known/lnurlp/citadel";
        let payment_category = Payment::from_str(lnurlp).unwrap();
        let expected = LnUrl::from_url(lnurlp.to_string());
        assert_eq!(payment_category.kind(), PaymentKind::LnUrl);
        assert_eq!(payment_category.lnurl().unwrap().lnurl(), Some(&expected));
        assert!(payment_category.lnurl().unwrap().lud16().is_none());
        assert!(matches!(
            payment_category,
            Payment::LnUrlCat(LnUrlIdentifier::LnUrl(lnurl)) if lnurl == expected
        ));

        let lnurl_email = "citadel@geyser.fund";
        let payment_category =
            Payment::from_str(format!("lightning:{lnurl_email}").as_str()).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LnUrl);
        assert!(payment_category.lnurl().unwrap().lnurl().is_none());
        assert_eq!(payment_category.lnurl().unwrap().lud16(), Some(lnurl_email));
        assert!(matches!(
            payment_category,
            Payment::LnUrlCat(LnUrlIdentifier::Lud16(lud16)) if lud16 == lnurl_email
        ));

        let address =
            "VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag";
        let asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
        let amount = "0.00000010";
        let liquid_bip21 = format!("liquidnetwork:{address}?amount={amount}&assetid={asset}");
        let payment_category = Payment::from_str(&liquid_bip21).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LiquidBip21);
        let bip21_ref = payment_category.liquid_bip21().unwrap();
        assert_eq!(
            bip21_ref.address,
            elements::Address::from_str(address).unwrap()
        );
        assert_eq!(bip21_ref.asset, AssetId::from_str(asset).unwrap());
        assert_eq!(bip21_ref.satoshi, Some(10));
        assert!(payment_category.liquid_address().is_none());

        let address =
            "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m";
        let asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
        let amount = "0.00000010";
        let liquid_bip21 = format!("liquidtestnet:{address}?amount={amount}&assetid={asset}");
        let payment_category = Payment::from_str(&liquid_bip21).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LiquidBip21);
        let bip21_ref = payment_category.liquid_bip21().unwrap();
        assert_eq!(
            bip21_ref.address,
            elements::Address::from_str(address).unwrap()
        );
        assert_eq!(bip21_ref.asset, AssetId::from_str(asset).unwrap());
        assert_eq!(bip21_ref.satoshi, Some(10));
        assert!(payment_category.liquid_address().is_none());
    }

    #[test]
    fn test_parse_liquid_bip21() {
        let liquid_bip21 = "liquidnetwork:VJL67HETqJCTg8Jak34N4RQaZD8HopbuhiU6F5kdo4d8QBJKTNJY3N1ictsXc1KAVNpaTEuCEoUCAzEj?amount=0.00001000&assetid=6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
        let _payment_category = Payment::from_str(liquid_bip21).unwrap();
    }

    #[test]
    fn test_parse_liquid_bip21_only_asset() {
        let liquid_bip21 = "liquidnetwork:VJLGMJ6mExPjidy3evXx5qjfbL4G4iVnyLLmaCTdzSUna3NbXrAR6MheMk3xcSGs3A1TYuJn1C8dQ8W5?assetid=ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
        let _payment_category = Payment::from_str(liquid_bip21).unwrap();
    }

    #[test]
    fn test_parse_liquid_bip21_missing_asset() {
        let liquid_bip21 = "liquidnetwork:VJL67HETqJCTg8Jak34N4RQaZD8HopbuhiU6F5kdo4d8QBJKTNJY3N1ictsXc1KAVNpaTEuCEoUCAzEj?amount=0.00001000";
        let err = Payment::from_str(liquid_bip21).unwrap_err();
        assert_eq!(
            err,
            "Invalid payment request: assetID needs to be specified".into()
        );
    }

    #[test]
    fn test_parse_no_schema() {
        let bitcoin_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let result = parse_no_schema(bitcoin_address).unwrap();
        let expected =
            bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(bitcoin_address)
                .unwrap();
        assert_eq!(result.kind(), PaymentKind::BitcoinAddress);
        assert_eq!(result.bitcoin_address(), Some(&expected));
        assert!(result.liquid_address().is_none());
        assert!(matches!(
            result,
            Payment::BitcoinAddress(addr) if addr == expected
        ));

        let bitcoin_segwit_address = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";
        let result = parse_no_schema(bitcoin_segwit_address).unwrap();
        let expected = bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(
            bitcoin_segwit_address,
        )
        .unwrap();
        assert_eq!(result.kind(), PaymentKind::BitcoinAddress);
        assert_eq!(result.bitcoin_address(), Some(&expected));
        assert!(matches!(
            result,
            Payment::BitcoinAddress(addr) if addr == expected
        ));

        let bitcoin_segwit_address_upper = bitcoin_segwit_address.to_uppercase();
        let result = parse_no_schema(&bitcoin_segwit_address_upper).unwrap();
        let expected = bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(
            &bitcoin_segwit_address_upper,
        )
        .unwrap();
        assert_eq!(result.kind(), PaymentKind::BitcoinAddress);
        assert_eq!(result.bitcoin_address(), Some(&expected));
        assert!(matches!(
            result,
            Payment::BitcoinAddress(addr) if addr == expected
        ));

        let liquid_address = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let result = parse_no_schema(liquid_address).unwrap();
        let expected = elements::Address::from_str(liquid_address).unwrap();
        assert_eq!(result.kind(), PaymentKind::LiquidAddress);
        assert_eq!(result.liquid_address(), Some(&expected));
        assert!(result.bitcoin_address().is_none());
        assert!(matches!(
            result,
            Payment::LiquidAddress(addr) if addr == expected
        ));

        let liquid_address_upper = liquid_address.to_uppercase();
        let result = parse_no_schema(&liquid_address_upper).unwrap();
        let expected = elements::Address::from_str(&liquid_address_upper).unwrap();
        assert_eq!(result.kind(), PaymentKind::LiquidAddress);
        assert_eq!(result.liquid_address(), Some(&expected));
        assert!(matches!(
            result,
            Payment::LiquidAddress(addr) if addr == expected
        ));

        let lightning_invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let result = parse_no_schema(lightning_invoice).unwrap();
        let expected = Bolt11Invoice::from_str(lightning_invoice).unwrap();
        assert_eq!(result.kind(), PaymentKind::LightningInvoice);
        assert_eq!(result.lightning_invoice(), Some(&expected));
        assert!(result.lightning_offer().is_none());
        assert!(matches!(
            result,
            Payment::LightningInvoice(invoice) if invoice == expected
        ));

        let lightning_invoice_upper = lightning_invoice.to_uppercase();
        let result = parse_no_schema(&lightning_invoice_upper).unwrap();
        let expected = Bolt11Invoice::from_str(&lightning_invoice_upper).unwrap();
        assert_eq!(result.kind(), PaymentKind::LightningInvoice);
        assert_eq!(result.lightning_invoice(), Some(&expected));
        assert!(matches!(
            result,
            Payment::LightningInvoice(invoice) if invoice == expected
        ));

        let offer = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";
        let result = parse_no_schema(offer).unwrap();
        let expected = Offer::from_str(offer).unwrap();
        assert_eq!(result.kind(), PaymentKind::LightningOffer);
        assert_eq!(result.lightning_offer(), Some(&expected));
        assert!(result.lightning_invoice().is_none());
        assert!(matches!(
            result,
            Payment::LightningOffer(offer) if *offer == expected
        ));

        let offer_upper = offer.to_uppercase();
        let result = parse_no_schema(&offer_upper).unwrap();
        let expected = Offer::from_str(&offer_upper).unwrap();
        assert_eq!(result.kind(), PaymentKind::LightningOffer);
        assert_eq!(result.lightning_offer(), Some(&expected));
        assert!(matches!(
            result,
            Payment::LightningOffer(offer) if *offer == expected
        ));

        let lnurl =
            "lnurl1dp68gurn8ghj7mn0wd68yene9e3k7mf0d3h82unvwqhkzurf9amrztmvde6hymp0xge7pp36";
        let result = parse_no_schema(lnurl).unwrap();
        let expected = LnUrl::from_str(lnurl).unwrap();
        assert_eq!(result.kind(), PaymentKind::LnUrl);
        assert_eq!(result.lnurl().unwrap().lnurl(), Some(&expected));
        assert!(result.lnurl().unwrap().lud16().is_none());
        assert!(result.lightning_invoice().is_none());
        assert!(matches!(
            result,
            Payment::LnUrlCat(LnUrlIdentifier::LnUrl(lnurl)) if lnurl == expected
        ));

        let lnurl_upper = lnurl.to_uppercase();
        let result = parse_no_schema(&lnurl_upper).unwrap();
        let expected = LnUrl::from_str(&lnurl_upper).unwrap();
        assert_eq!(result.kind(), PaymentKind::LnUrl);
        assert_eq!(result.lnurl().unwrap().lnurl(), Some(&expected));
        assert!(result.lnurl().unwrap().lud16().is_none());
        assert!(matches!(
            result,
            Payment::LnUrlCat(LnUrlIdentifier::LnUrl(lnurl)) if lnurl == expected
        ));

        let lud16 = "citadel@geyser.fund";
        let result = parse_no_schema(lud16).unwrap();
        assert_eq!(result.kind(), PaymentKind::LnUrl);
        assert!(result.lnurl().unwrap().lnurl().is_none());
        assert_eq!(result.lnurl().unwrap().lud16(), Some(lud16));
        assert!(matches!(
            result,
            Payment::LnUrlCat(LnUrlIdentifier::Lud16(identifier)) if identifier == lud16
        ));

        let bip353 = "₿matt@mattcorallo.com";
        let result = parse_no_schema(bip353).unwrap();
        let expected = "matt@mattcorallo.com";
        assert_eq!(result.kind(), PaymentKind::Bip353);
        assert_eq!(result.bip353(), Some(expected));
        assert!(result.lnurl().is_none());
        assert!(matches!(
            result,
            Payment::Bip353(bip353) if bip353 == expected
        ));
    }

    #[tokio::test]
    async fn test_resolve_bip353_to_offer() {
        let offer = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";
        let resolver = TestHrnResolver {
            result: "bitcoin:?lno=lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv",
        };

        let resolved = resolve_bip353_with_resolver("matt@example.com", &resolver)
            .await
            .unwrap();

        assert_eq!(resolved, Offer::from_str(offer).unwrap());
    }

    #[tokio::test]
    async fn test_resolve_non_bip353_fails() {
        let payment = Payment::from_str("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap();
        let err = payment.resolve_bip353().await.unwrap_err();

        assert_eq!(err, Error::ExpectedKind(PaymentKind::Bip353));
    }

    #[tokio::test]
    async fn test_resolve_non_lnurl_fails() {
        let payment = Payment::from_str("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap();
        let err = payment.resolve_lnurl_info().await.unwrap_err();

        assert_eq!(err, Error::ExpectedKind(PaymentKind::LnUrl));
    }

    #[test]
    fn test_plain_email_parsed_as_lud16() {
        let email = "user@example.com";
        let res = parse_no_schema(email).unwrap();
        assert_eq!(res.kind(), PaymentKind::LnUrl);
        assert!(res.lnurl().unwrap().lnurl().is_none());
        assert_eq!(res.lnurl().unwrap().lud16(), Some(email));

        let res_with_prefix = Payment::from_str(&format!("lightning:{email}")).unwrap();
        assert_eq!(res_with_prefix.kind(), PaymentKind::LnUrl);
        assert!(res_with_prefix.lnurl().unwrap().lnurl().is_none());
        assert_eq!(res_with_prefix.lnurl().unwrap().lud16(), Some(email));
    }

    #[test]
    fn test_is_email_debug_only_local_domains() {
        assert!(is_email("user@example.com"));

        if cfg!(debug_assertions) {
            assert!(is_email("user@localhost"));
            assert!(is_email("user@127.0.0.1"));
            assert!(is_email("user@127.0.0.1:3000"));
        } else {
            assert!(!is_email("user@localhost"));
            assert!(!is_email("user@127.0.0.1"));
            assert!(!is_email("user@127.0.0.1:3000"));
        }
    }

    #[tokio::test]
    async fn test_lnurl_resolution_with_mock() {
        let mut server = mockito::Server::new_async().await;

        let lnurl_path = "/.well-known/lnurlp/user";
        let callback_path = "/callback";
        let metadata = "[[\"text/plain\",\"test metadata\"]]";

        let _m1 = server
            .mock("GET", lnurl_path)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"{{"tag":"payRequest","callback":"{}{}","minSendable":1000,"maxSendable":1000000,"metadata":"{}"}}"#,
                server.url(),
                callback_path,
                metadata.replace("\"", "\\\"")
            ))
            .create_async()
            .await;

        let invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let _m2 = server
            .mock("GET", callback_path)
            .match_query(mockito::Matcher::UrlEncoded(
                "amount".into(),
                "10000".into(),
            ))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(r#"{{"pr":"{}"}}"#, invoice))
            .create_async()
            .await;

        // 1. Test resolve_lnurl_info.
        // Local LUD-16 parsing is enabled only with debug assertions, so fall back to a
        // direct LNURL URL in release-style test builds.
        let payment = if cfg!(debug_assertions) {
            let addr = format!("lightning:user@{}", server.host_with_port());
            Payment::from_str(&addr).unwrap()
        } else {
            Payment::LnUrlCat(LnUrlIdentifier::LnUrl(LnUrl::from_url(format!(
                "{}{}",
                server.url(),
                lnurl_path
            ))))
        };
        let info = payment.resolve_lnurl_info().await.unwrap();

        assert_eq!(info.tag, "payRequest");
        assert_eq!(info.min_sendable, 1000);
        assert_eq!(info.max_sendable, 1000000);
        assert_eq!(info.metadata, metadata);
        assert!(info.callback.contains(callback_path));

        // 2. Test fetch_lnurl_invoice
        let amount_sats = 10; // 10000 msat
        let invoice_payment = Payment::fetch_lnurl_invoice(&info, amount_sats)
            .await
            .unwrap();
        assert_eq!(
            invoice_payment.lightning_invoice().unwrap().to_string(),
            invoice
        );

        // 3. Test out of range amount
        let err = Payment::fetch_lnurl_invoice(&info, 1001) // 1001000 msat > 1000000
            .await
            .unwrap_err();
        assert!(err.to_string().contains("out of range"));
    }

    #[tokio::test]
    #[ignore = "requires live DNS access"]
    async fn test_resolve_bip353_matt() {
        let payment = Payment::from_str("₿matt@mattcorallo.com").unwrap();
        let result = payment.resolve_bip353().await.unwrap();
        assert!(matches!(result, Payment::LightningOffer(_)));
        let offer = result.lightning_offer().unwrap();
        assert_eq!(offer.to_string(), "lno1zr5qyugqgskrk70kqmuq7v3dnr2fnmhukps9n8hut48vkqpqnskt2svsqwjakp7k6pyhtkuxw7y2kqmsxlwruhzqv0zsnhh9q3t9xhx39suc6qsr07ekm5esdyum0w66mnx8vdquwvp7dp5jp7j3v5cp6aj0w329fnkqqv60q96sz5nkrc5r95qffx002q53tqdk8x9m2tmt85jtpmcycvfnrpx3lr45h2g7na3sec7xguctfzzcm8jjqtj5ya27te60j03vpt0vq9tm2n9yxl2hngfnmygesa25s4u4zlxewqpvp94xt7rur4rhxunwkthk9vly3lm5hh0pqv4aymcqejlgssnlpzwlggykkajp7yjs5jvr2agkyypcdlj280cy46jpynsezrcj2kwa2lyr8xvd6lfkph4xrxtk2xc3lpq");
    }

    #[tokio::test]
    #[ignore = "requires live network access"]
    async fn test_resolve_lnurl_info() {
        let payment = Payment::from_str("citadel@geyser.fund").unwrap();
        let info = payment.resolve_lnurl_info().await.unwrap();
        assert_eq!(info.tag, "payRequest");
        assert!(info.callback.contains("geyser.fund"));
        assert!(info.min_sendable > 0);
        assert!(info.max_sendable >= info.min_sendable);

        let amount = info.min_sendable;
        let invoice_payment = Payment::fetch_lnurl_invoice(&info, amount).await.unwrap();
        assert!(matches!(invoice_payment, Payment::LightningInvoice(_)));
    }
}
