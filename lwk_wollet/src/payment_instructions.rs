use elements::{bitcoin, AssetId};

#[non_exhaustive]
enum PaymentCategory {
    BitcoinAddress(bitcoin::Address), // just the address, or bitcoin:<address>
    LiquidAddress(elements::Address), // just the address, or liquidnetwork:<address> or liquidtestnet:<address>
    LightningInvoice(Bolt11Invoice),  // just the invoice or lightning:<invoice>
    LightningOffer(Offer),            // just the bolt12 or lightning:<bolt12>
    LnUrl(String), // just lnurl or lightning:<lnurl>https://doc.rust-lang.org/reference/attributes/type_system.html
    Bip353(String), // ₿matt@mattcorallo.com
    Bip21(bip21::Uri), // bitcoin:
    LiquidBip21(elements::Address, AssetId, u64), // liquidnetwork: liquidtestnet:
}

impl FromStr for PaymentCategory {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

impl Display for PaymentCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
