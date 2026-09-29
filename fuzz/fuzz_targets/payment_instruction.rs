#![no_main]

use std::str::FromStr;

use libfuzzer_sys::fuzz_target;
use lwk_payment_instructions::{Payment, PaymentKind};

const MAX_INPUT_BYTES: usize = 4096;
const BITCOIN_ADDRESS: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
const LIQUID_ADDRESS: &str =
    "VJLDJCJZja8GZNBkLFAHWSNwuxMrzs1BpX1CAUqvfwgtRtDdVtPFWiQwnYMf76rMamsUgFFJVgf36eag";
const ASSET_ID: &str = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";

fn parse_and_check(input: &str) {
    let Ok(payment) = Payment::from_str(input) else {
        return;
    };
    let kind = payment.kind();

    match &payment {
        Payment::BitcoinAddress(_) => {
            assert_eq!(kind, PaymentKind::BitcoinAddress);
            assert!(payment.bitcoin_address().is_some());
        }
        Payment::LiquidAddress(_) => {
            assert_eq!(kind, PaymentKind::LiquidAddress);
            assert!(payment.liquid_address().is_some());
        }
        Payment::LightningInvoice(_) => {
            assert_eq!(kind, PaymentKind::LightningInvoice);
            assert!(payment.lightning_invoice().is_some());
        }
        Payment::LightningOffer(_) => {
            assert_eq!(kind, PaymentKind::LightningOffer);
            assert!(payment.lightning_offer().is_some());
        }
        Payment::LnUrlCat(_) => {
            assert_eq!(kind, PaymentKind::LnUrl);
            assert!(payment.lnurl().is_some());
        }
        Payment::Bip353(_) => {
            assert_eq!(kind, PaymentKind::Bip353);
            assert!(payment.bip353().is_some());
        }
        Payment::Bip21(uri) => {
            assert_eq!(kind, PaymentKind::Bip21);
            assert!(payment.bip21().is_some());

            let _ = uri.address();
            let _ = uri.amount();
            let _ = uri.label();
            let _ = uri.message();
            let _ = uri.lightning();
            let _ = uri.offer();
            let _ = uri.payjoin();
            let _ = uri.payjoin_output_substitution();
            let _ = uri.silent_payment_address();
            let _ = uri.ark();

            let reparsed = Payment::from_str(uri.as_str()).expect("BIP21 reparses");
            assert_eq!(reparsed.kind(), kind);
        }
        Payment::Bip321(uri) => {
            assert_eq!(kind, PaymentKind::Bip321);
            assert!(payment.bip321().is_some());

            let _ = uri.address();
            let _ = uri.amount();
            let _ = uri.label();
            let _ = uri.message();
            let _ = uri.lightning();
            let _ = uri.offer();
            let _ = uri.payjoin();
            let _ = uri.payjoin_output_substitution();
            let _ = uri.silent_payment_address();
            let _ = uri.ark();

            let reparsed = Payment::from_str(uri.as_str()).expect("BIP321 reparses");
            assert_eq!(reparsed.kind(), kind);
        }
        Payment::LiquidBip21(_) => {
            assert_eq!(kind, PaymentKind::LiquidBip21);
            assert!(payment.liquid_bip21().is_some());
        }
        _ => {}
    }
}

fn exercise(input: &str) {
    parse_and_check(input);

    for schema in [
        "bitcoin:",
        "BITCOIN:",
        "liquidnetwork:",
        "liquidtestnet:",
        "lightning:",
        "lnurlp:",
    ] {
        parse_and_check(&format!("{schema}{input}"));
    }

    parse_and_check(&format!("bitcoin:{BITCOIN_ADDRESS}?{input}"));
    parse_and_check(&format!("bitcoin:?ark={input}"));
    parse_and_check(&format!(
        "liquidnetwork:{LIQUID_ADDRESS}?assetid={ASSET_ID}&amount={input}&{input}"
    ));
    parse_and_check(&format!("lnurlp://example.com/{input}"));
    parse_and_check(&format!("{input}@example.com"));
    parse_and_check(&format!("₿{input}@example.com"));
}

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_INPUT_BYTES)];

    if let Ok(input) = std::str::from_utf8(data) {
        exercise(input);
    } else {
        exercise(&String::from_utf8_lossy(data));
    }
});
