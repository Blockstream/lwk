use std::str::FromStr;

use elements::{
    bitcoin::{self, address::NetworkUnchecked},
    AddressParams, AssetId,
};
use lightning::offers::offer::Offer;
use lightning_invoice::Bolt11Invoice;
use lnurl::lnurl::LnUrl;

use crate::bip21::Bip21;

mod bip21;

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
    LiquidBip21,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct LiquidBip21 {
    pub address: elements::Address,
    pub asset: AssetId,
    pub amount: u64,
}

#[allow(dead_code)]
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum Payment {
    BitcoinAddress(bitcoin::Address<bitcoin::address::NetworkUnchecked>), // just the address, or bitcoin:<address>
    LiquidAddress(elements::Address), // just the address, or liquidnetwork:<address> or liquidtestnet:<address>
    LightningInvoice(Bolt11Invoice),  // just the invoice or lightning:<invoice>
    LightningOffer(Box<Offer>),       // just the bolt12 or lightning:<bolt12>
    LnUrlCat(LnUrl),                  // just lnurl or lightning:<lnurl> or lnurlp://<url>
    Bip353(String),                   // ₿matt@mattcorallo.com
    Bip21(Bip21),                     // bitcoin:
    LiquidBip21(LiquidBip21),         // liquidnetwork: liquidtestnet:
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

    pub fn lnurl(&self) -> Option<&LnUrl> {
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

    pub fn bip21(&self) -> Option<&str> {
        match self {
            Payment::Bip21(uri) => Some(uri.as_str()),
            _ => None,
        }
    }

    pub fn liquid_bip21(&self) -> Option<&LiquidBip21> {
        match self {
            Payment::LiquidBip21(bip21) => Some(bip21),
            _ => None,
        }
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
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
            _ => Err(format!("Invalid schema: {s}")),
        }
    }
}

impl FromStr for Payment {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
    cat: Result<Payment, String>,
    s: &str,
) -> Result<Payment, String> {
    use Payment::*;
    use Schema::*;
    match (schema, cat) {
        (Bitcoin, Ok(cat @ BitcoinAddress(_))) => Ok(cat),
        (Bitcoin, Err(_)) => {
            // Validate it's a correct bip21::Uri, then store as String
            let bip21 = bip21::Bip21::from_str(s)?;
            Ok(Bip21(bip21))
        }

        (LiquidNetwork, Ok(ref cat @ LiquidAddress(ref a))) => {
            if a.params == &AddressParams::LIQUID {
                Ok(cat.clone())
            } else {
                Err(format!(
                    "Using liquidnetwork schema with non-mainnet address: {s}"
                ))
            }
        }
        (LiquidNetwork, Err(_)) => parse_liquid_bip21(s, true),
        (LiquidTestnet, Ok(ref cat @ LiquidAddress(ref a))) => {
            if a.params != &AddressParams::LIQUID {
                Ok(cat.clone())
            } else {
                Err(format!(
                    "Using liquidtestnet schema with mainnet address: {s}"
                ))
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
                let lnurl = LnUrl::from_url(rest.to_string());
                Ok(LnUrlCat(lnurl))
            } else {
                Err(format!("Invalid lightning schema: {s}"))
            }
        }
        (LnUrlP, _) => {
            // lnurlp://<url> can be an lnurl
            url::Url::from_str(s).map_err(|e| e.to_string())?;
            let lnurl = LnUrl::from_url(s.to_string());
            Ok(LnUrlCat(lnurl))
        }
        _ => Err(format!("Invalid schema: {s}")),
    }
}

fn parse_liquid_bip21(s: &str, is_mainnet: bool) -> Result<Payment, String> {
    let url = url::Url::from_str(s).map_err(|e| e.to_string())?;

    let address_str = url.path();
    let address = elements::Address::from_str(address_str).map_err(|e| e.to_string())?;

    let is_liquid_mainnet = address.params == &AddressParams::LIQUID;
    if is_mainnet && !is_liquid_mainnet {
        return Err(format!(
            "Using liquidnetwork schema with non-mainnet address: {s}"
        ));
    }
    if !is_mainnet && is_liquid_mainnet {
        return Err(format!(
            "Using liquidtestnet schema with mainnet address: {s}"
        ));
    }

    let asset_str = url
        .query_pairs()
        .find(|(key, _)| key == "assetid")
        .map(|(_, value)| value)
        .ok_or_else(|| "error".to_string())?;
    let asset = AssetId::from_str(&asset_str).map_err(|e| e.to_string())?;
    let amount_str = url
        .query_pairs()
        .find(|(key, _)| key == "amount")
        .map(|(_, value)| value)
        .ok_or_else(|| "error".to_string())?;
    let amount = amount_str.parse::<u64>().map_err(|e| e.to_string())?;

    Ok(Payment::LiquidBip21(LiquidBip21 {
        address,
        asset,
        amount,
    }))
}

fn parse_no_schema(s: &str) -> Result<Payment, String> {
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
        return Ok(Payment::LnUrlCat(lnurl));
    }
    if s.starts_with("₿") {
        let rest = s.chars().skip(1).collect::<String>();
        if is_email(&rest) {
            return Ok(Payment::Bip353(rest));
        }
    }
    Err(format!("Invalid payment category: {s}"))
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    // Basic checks: non-empty local part, domain has at least one dot with content after it
    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_with_schema_fails() {
        let payment_category = Payment::from_str("bitcoin:invalid_address").unwrap_err();
        assert_eq!(payment_category, "invalid BIP21 URI");

        // mixed case schema are not supported
        let payment_category =
            Payment::from_str("BITcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap_err();
        assert_eq!(payment_category, "Invalid schema: BITcoin");

        // valid mainnet address with testnet schema
        let payment_category = Payment::from_str("liquidtestnet:lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0").unwrap_err();
        assert_eq!(
            payment_category,
            "Using liquidtestnet schema with mainnet address: liquidtestnet:lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0"
        );

        // valid testnet address with mainnet schema
        let payment_category = Payment::from_str("liquidnetwork:tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m").unwrap_err();
        assert_eq!(
            payment_category,
            "Using liquidnetwork schema with non-mainnet address: liquidnetwork:tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m"
        );

        // valid testnet address with testnet schema
        let err = Payment::from_str("liquidtestnet:VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag?amount=10&assetid=ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2").unwrap_err();
        assert_eq!(err, "Using liquidtestnet schema with mainnet address: liquidtestnet:VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag?amount=10&assetid=ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2");
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
        assert_eq!(payment_category.bip21(), Some(bip21));
        assert!(payment_category.bitcoin_address().is_none());
        assert!(matches!(
            payment_category,
            Payment::Bip21(ref uri) if uri == bip21
        ));

        let bip21_upper = "BITCOIN:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50";
        let payment_category = Payment::from_str(bip21_upper).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::Bip21);
        assert_eq!(payment_category.bip21(), Some(bip21_upper));
        assert!(matches!(
            payment_category,
            Payment::Bip21(ref uri) if uri == bip21_upper
        ));

        let lnurl = "lnurl1dp68gurn8ghj7ctsdyhxwetewdjhytnxw4hxgtmvde6hymp0wpshj0mswfhk5etrw3ykg0f3xqcs2mcx97";
        let payment_category = Payment::from_str(&format!("lightning:{lnurl}")).unwrap();
        let expected = LnUrl::from_str(lnurl).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LnUrl);
        assert_eq!(payment_category.lnurl(), Some(&expected));
        assert!(payment_category.lightning_invoice().is_none());
        assert!(matches!(
            payment_category,
            Payment::LnUrlCat(lnurl) if lnurl == expected
        ));

        let lnurlp = "lnurlp://geyser.fund/.well-known/lnurlp/citadel";
        let payment_category = Payment::from_str(lnurlp).unwrap();
        let expected = LnUrl::from_url(lnurlp.to_string());
        assert_eq!(payment_category.kind(), PaymentKind::LnUrl);
        assert_eq!(payment_category.lnurl(), Some(&expected));
        assert!(matches!(
            payment_category,
            Payment::LnUrlCat(lnurl) if lnurl == expected
        ));

        let lnurl_email = "citadel@geyser.fund";
        let payment_category =
            Payment::from_str(format!("lightning:{lnurl_email}").as_str()).unwrap();
        let expected = LnUrl::from_url(lnurl_email.to_string());
        assert_eq!(payment_category.kind(), PaymentKind::LnUrl);
        assert_eq!(payment_category.lnurl(), Some(&expected));
        assert!(matches!(
            payment_category,
            Payment::LnUrlCat(lnurl) if lnurl == expected
        ));

        let address =
            "VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag";
        let asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
        let amount = 10;
        let liquid_bip21 = format!("liquidnetwork:{address}?amount={amount}&assetid={asset}");
        let payment_category = Payment::from_str(&liquid_bip21).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LiquidBip21);
        let bip21_ref = payment_category.liquid_bip21().unwrap();
        assert_eq!(
            bip21_ref.address,
            elements::Address::from_str(address).unwrap()
        );
        assert_eq!(bip21_ref.asset, AssetId::from_str(asset).unwrap());
        assert_eq!(bip21_ref.amount, amount);
        assert!(payment_category.liquid_address().is_none());

        let address =
            "tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m";
        let asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
        let amount = 10;
        let liquid_bip21 = format!("liquidtestnet:{address}?amount={amount}&assetid={asset}");
        let payment_category = Payment::from_str(&liquid_bip21).unwrap();
        assert_eq!(payment_category.kind(), PaymentKind::LiquidBip21);
        let bip21_ref = payment_category.liquid_bip21().unwrap();
        assert_eq!(
            bip21_ref.address,
            elements::Address::from_str(address).unwrap()
        );
        assert_eq!(bip21_ref.asset, AssetId::from_str(asset).unwrap());
        assert_eq!(bip21_ref.amount, amount);
        assert!(payment_category.liquid_address().is_none());
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
        assert_eq!(result.lnurl(), Some(&expected));
        assert!(result.lightning_invoice().is_none());
        assert!(matches!(
            result,
            Payment::LnUrlCat(lnurl) if lnurl == expected
        ));

        let lnurl_upper = lnurl.to_uppercase();
        let result = parse_no_schema(&lnurl_upper).unwrap();
        let expected = LnUrl::from_str(&lnurl_upper).unwrap();
        assert_eq!(result.kind(), PaymentKind::LnUrl);
        assert_eq!(result.lnurl(), Some(&expected));
        assert!(matches!(
            result,
            Payment::LnUrlCat(lnurl) if lnurl == expected
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
}
