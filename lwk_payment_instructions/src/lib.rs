use std::{fmt::Display, str::FromStr};

use bip21::NoExtras;
use elements::{
    bitcoin::{self, address::NetworkUnchecked},
    AddressParams, AssetId,
};
use lightning::offers::offer::Offer;
use lightning_invoice::Bolt11Invoice;
use lnurl::lnurl::LnUrl;

#[allow(dead_code)]
#[non_exhaustive]
#[derive(Clone, Debug)]
enum PaymentCategory<'a> {
    BitcoinAddress(bitcoin::Address<bitcoin::address::NetworkUnchecked>), // just the address, or bitcoin:<address>
    LiquidAddress(elements::Address), // just the address, or liquidnetwork:<address> or liquidtestnet:<address>
    LightningInvoice(Bolt11Invoice),  // just the invoice or lightning:<invoice>
    LightningOffer(Box<Offer>),       // just the bolt12 or lightning:<bolt12>
    LnUrlCat(LnUrl),                  // just lnurl or lightning:<lnurl> or lnurlp://<url>
    Bip353(String),                   // ₿matt@mattcorallo.com
    Bip21(bip21::Uri<'a, NetworkUnchecked, NoExtras>), // bitcoin:
    LiquidBip21 {
        address: elements::Address,
        asset: AssetId,
        amount: u64,
    }, // liquidnetwork: liquidtestnet:
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

impl Display for PaymentCategory<'_> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl FromStr for PaymentCategory<'_> {
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

fn parse_with_schema<'a>(
    schema: Schema,
    cat: Result<PaymentCategory<'a>, String>,
    s: &str,
) -> Result<PaymentCategory<'a>, String> {
    use PaymentCategory::*;
    use Schema::*;
    match (schema, cat) {
        (Bitcoin, Ok(cat @ BitcoinAddress(_))) => Ok(cat),
        (Bitcoin, Err(_)) => {
            let bip21_uri = bip21::Uri::from_str(s).map_err(|e| e.to_string())?;
            Ok(Bip21(bip21_uri))
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
        (LiquidTestnet, Ok(ref cat @ LiquidAddress(ref a))) => {
            if a.params != &AddressParams::LIQUID {
                Ok(cat.clone())
            } else {
                Err(format!(
                    "Using liquidtestnet schema with mainnet address: {s}"
                ))
            }
        }
        (Lightning, Ok(cat @ LightningInvoice(_))) => Ok(cat),
        (Lightning, Ok(cat @ LightningOffer(_))) => Ok(cat),
        (Lightning, Ok(cat @ LnUrlCat(_))) => Ok(cat),
        (Lightning, Err(_)) => {
            // lightning:<email> can be an lnurl
            let rest = &s[10..];
            if is_email(rest) {
                let lnurl = LnUrl::from_url(rest.to_string());
                return Ok(LnUrlCat(lnurl));
            } else {
                todo!("{s}");
            }
        }
        (LnUrlP, _) => {
            // lnurlp://<url> can be an lnurl
            url::Url::from_str(s).map_err(|e| e.to_string())?;
            let lnurl = LnUrl::from_url(s.to_string());
            Ok(LnUrlCat(lnurl))
        }
        _ => todo!("{s}"),
    }
}

fn parse_no_schema<'a>(s: &str) -> Result<PaymentCategory<'a>, String> {
    if let Ok(bitcoin_address) = bitcoin::Address::from_str(s) {
        return Ok(PaymentCategory::BitcoinAddress(bitcoin_address));
    }
    if let Ok(liquid_address) = elements::Address::from_str(s) {
        return Ok(PaymentCategory::LiquidAddress(liquid_address));
    }
    if let Ok(lightning_invoice) = Bolt11Invoice::from_str(s) {
        return Ok(PaymentCategory::LightningInvoice(lightning_invoice));
    }
    if let Ok(lightning_offer) = Offer::from_str(s) {
        return Ok(PaymentCategory::LightningOffer(Box::new(lightning_offer)));
    }
    if let Ok(lnurl) = LnUrl::from_str(s) {
        return Ok(PaymentCategory::LnUrlCat(lnurl));
    }
    if s.starts_with("₿") {
        let rest = s.chars().skip(1).collect::<String>();
        if is_email(&rest) {
            return Ok(PaymentCategory::Bip353(rest));
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
        let payment_category = PaymentCategory::from_str("bitcoin:invalid_address").unwrap_err();
        assert_eq!(payment_category, "invalid BIP21 URI");

        // mixed case schema are not supported
        let payment_category =
            PaymentCategory::from_str("BITcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa").unwrap_err();
        assert_eq!(payment_category, "Invalid schema: BITcoin");

        // valid mainnet address with testnet schema
        let payment_category = PaymentCategory::from_str("liquidtestnet:lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0").unwrap_err();
        assert_eq!(
            payment_category,
            "Using liquidtestnet schema with mainnet address: liquidtestnet:lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0"
        );

        // valid testnet address with mainnet schema
        let payment_category = PaymentCategory::from_str("liquidnetwork:tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m").unwrap_err();
        assert_eq!(
            payment_category,
            "Using liquidnetwork schema with non-mainnet address: liquidnetwork:tlq1qq02egjncr8g4qn890mrw3jhgupwqymekv383lwpmsfghn36hac5ptpmeewtnftluqyaraa56ung7wf47crkn5fjuhk422d68m"
        );
    }

    #[test]
    fn test_parse_with_schema() {
        let bitcoin_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let payment_category =
            PaymentCategory::from_str(&format!("bitcoin:{bitcoin_address}")).unwrap();
        let expected =
            bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(bitcoin_address)
                .unwrap();
        assert!(matches!(
            payment_category,
            PaymentCategory::BitcoinAddress(addr) if addr == expected
        ));
        let payment_category =
            PaymentCategory::from_str(&format!("BITCOIN:{bitcoin_address}")).unwrap();
        let expected =
            bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(bitcoin_address)
                .unwrap();
        assert!(matches!(
            payment_category,
            PaymentCategory::BitcoinAddress(addr) if addr == expected
        ));

        let liquid_address = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let payment_category =
            PaymentCategory::from_str(&format!("liquidnetwork:{liquid_address}")).unwrap();
        let expected = elements::Address::from_str(liquid_address).unwrap();
        assert!(matches!(
            payment_category,
            PaymentCategory::LiquidAddress(addr) if addr == expected
        ));

        let lightning_invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let payment_category =
            PaymentCategory::from_str(&format!("lightning:{lightning_invoice}")).unwrap();
        let expected = Bolt11Invoice::from_str(lightning_invoice).unwrap();
        assert!(matches!(
            payment_category,
            PaymentCategory::LightningInvoice(invoice) if invoice == expected
        ));

        let bolt12 = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";
        let payment_category = PaymentCategory::from_str(&format!("lightning:{bolt12}")).unwrap();
        let expected = Offer::from_str(bolt12).unwrap();
        assert!(matches!(
            payment_category,
            PaymentCategory::LightningOffer(offer) if *offer == expected
        ));

        let bip21 = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50";
        let payment_category = PaymentCategory::from_str(bip21).unwrap();
        if let PaymentCategory::Bip21(uri) = payment_category {
            assert_eq!(uri.clone().assume_checked().to_string(), bip21);
        } else {
            panic!("Expected PaymentCategory::Bip21");
        }

        let bip21_upper = "BITCOIN:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50";
        let payment_category = PaymentCategory::from_str(bip21_upper).unwrap();
        if let PaymentCategory::Bip21(uri) = payment_category {
            assert_eq!(uri.clone().assume_checked().to_string(), bip21); // lower cased when displayed
        } else {
            panic!("Expected PaymentCategory::Bip21");
        }

        let lnurl = "lnurl1dp68gurn8ghj7ctsdyhxwetewdjhytnxw4hxgtmvde6hymp0wpshj0mswfhk5etrw3ykg0f3xqcs2mcx97";
        let payment_category = PaymentCategory::from_str(&format!("lightning:{lnurl}")).unwrap();
        let expected = LnUrl::from_str(lnurl).unwrap();
        assert!(matches!(
            payment_category,
            PaymentCategory::LnUrlCat(lnurl) if lnurl == expected
        ));

        let lnurlp = "lnurlp://geyser.fund/.well-known/lnurlp/citadel";
        let payment_category = PaymentCategory::from_str(lnurlp).unwrap();
        let expected = LnUrl::from_url(lnurlp.to_string());
        assert!(matches!(
            payment_category,
            PaymentCategory::LnUrlCat(lnurl) if lnurl == expected
        ));

        let lnurl_email = "citadel@geyser.fund";
        let payment_category =
            PaymentCategory::from_str(format!("lightning:{lnurl_email}").as_str()).unwrap();
        let expected = LnUrl::from_url(lnurl_email.to_string());
        assert!(matches!(
            payment_category,
            PaymentCategory::LnUrlCat(lnurl) if lnurl == expected
        ));
    }

    #[test]
    fn test_parse_no_schema() {
        let bitcoin_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let result = parse_no_schema(bitcoin_address).unwrap();
        let expected =
            bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(bitcoin_address)
                .unwrap();
        assert!(matches!(
            result,
            PaymentCategory::BitcoinAddress(addr) if addr == expected
        ));

        let bitcoin_segwit_address = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";
        let result = parse_no_schema(bitcoin_segwit_address).unwrap();
        let expected = bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(
            bitcoin_segwit_address,
        )
        .unwrap();
        assert!(matches!(
            result,
            PaymentCategory::BitcoinAddress(addr) if addr == expected
        ));

        let bitcoin_segwit_address_upper = bitcoin_segwit_address.to_uppercase();
        let result = parse_no_schema(&bitcoin_segwit_address_upper).unwrap();
        let expected = bitcoin::Address::<bitcoin::address::NetworkUnchecked>::from_str(
            &bitcoin_segwit_address_upper,
        )
        .unwrap();
        assert!(matches!(
            result,
            PaymentCategory::BitcoinAddress(addr) if addr == expected
        ));

        let liquid_address = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let result = parse_no_schema(liquid_address).unwrap();
        let expected = elements::Address::from_str(liquid_address).unwrap();
        assert!(matches!(
            result,
            PaymentCategory::LiquidAddress(addr) if addr == expected
        ));

        let liquid_address_upper = liquid_address.to_uppercase();
        let result = parse_no_schema(&liquid_address_upper).unwrap();
        let expected = elements::Address::from_str(&liquid_address_upper).unwrap();
        assert!(matches!(
            result,
            PaymentCategory::LiquidAddress(addr) if addr == expected
        ));

        let lightning_invoice = "lnbc23230n1p5sxxunsp5tep5yrw63cy3tk74j3hpzqzhhzwe806wk0apjfsfn5x9wmpkzkdspp5z4f40v2whks0aj3kx4zuwrrem094pna4ehutev2p63djtff02a2sdquf35kw6r5de5kueeqwpshjmt9de6qxqyp2xqcqzxrrzjqf6rgswuygn5qr0p5dt2mvklrrcz6yy8pnzqr3eq962tqwprpfrzkzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2g9qyysgqqufsg7s6qcmfmjxvkf0ulupufr0yfqeajnv3mvtyqzz2rfwre2796rnkzsw44lw3nja5frg4w4m59xqlwwu774h4f79ysm05uugckugqdf84yl";
        let result = parse_no_schema(lightning_invoice).unwrap();
        let expected = Bolt11Invoice::from_str(lightning_invoice).unwrap();
        assert!(matches!(
            result,
            PaymentCategory::LightningInvoice(invoice) if invoice == expected
        ));

        let lightning_invoice_upper = lightning_invoice.to_uppercase();
        let result = parse_no_schema(&lightning_invoice_upper).unwrap();
        let expected = Bolt11Invoice::from_str(&lightning_invoice_upper).unwrap();
        assert!(matches!(
            result,
            PaymentCategory::LightningInvoice(invoice) if invoice == expected
        ));

        let offer = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";
        let result = parse_no_schema(offer).unwrap();
        let expected = Offer::from_str(offer).unwrap();
        assert!(matches!(
            result,
            PaymentCategory::LightningOffer(offer) if *offer == expected
        ));

        let offer_upper = offer.to_uppercase();
        let result = parse_no_schema(&offer_upper).unwrap();
        let expected = Offer::from_str(&offer_upper).unwrap();
        assert!(matches!(
            result,
            PaymentCategory::LightningOffer(offer) if *offer == expected
        ));

        let lnurl =
            "lnurl1dp68gurn8ghj7mn0wd68yene9e3k7mf0d3h82unvwqhkzurf9amrztmvde6hymp0xge7pp36";
        let result = parse_no_schema(lnurl).unwrap();
        let expected = LnUrl::from_str(lnurl).unwrap();
        assert!(matches!(
            result,
            PaymentCategory::LnUrlCat(lnurl) if lnurl == expected
        ));

        let lnurl_upper = lnurl.to_uppercase();
        let result = parse_no_schema(&lnurl_upper).unwrap();
        let expected = LnUrl::from_str(&lnurl_upper).unwrap();
        assert!(matches!(
            result,
            PaymentCategory::LnUrlCat(lnurl) if lnurl == expected
        ));

        let bip353 = "₿matt@mattcorallo.com";
        let result = parse_no_schema(bip353).unwrap();
        let expected = "matt@mattcorallo.com";
        assert!(matches!(
            result,
            PaymentCategory::Bip353(bip353) if bip353 == expected
        ));
    }
}
