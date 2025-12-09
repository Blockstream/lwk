use bitcoin_payment_instructions::hrn_resolution::HrnResolver;
use elements::bitcoin::Network;

enum PaymentInstruction {
    BitcoinPaymentInstrunction(bitcoin_payment_instructions::PaymentInstructions),
}

impl PaymentInstruction {
    async fn parse(instructions: &str) -> Result<Self, ()> {
        let hrn_resolver = NoOpHrnResolver;
        let network = Network::Bitcoin;
        let inner = bitcoin_payment_instructions::PaymentInstructions::parse(
            instructions,
            network,
            &hrn_resolver,
            false,
        )
        .await
        .map_err(|_| ())?;

        Ok(PaymentInstruction::BitcoinPaymentInstrunction(inner))
    }
}

struct NoOpHrnResolver;
impl HrnResolver for NoOpHrnResolver {
    fn resolve_hrn<'a>(
        &'a self,
        hrn: &'a bitcoin_payment_instructions::hrn_resolution::HumanReadableName,
    ) -> bitcoin_payment_instructions::hrn_resolution::HrnResolutionFuture<'a> {
        todo!()
    }

    fn resolve_lnurl<'a>(
        &'a self,
        url: &'a str,
    ) -> bitcoin_payment_instructions::hrn_resolution::HrnResolutionFuture<'a> {
        todo!()
    }

    fn resolve_lnurl_to_invoice<'a>(
        &'a self,
        callback_url: String,
        amount: bitcoin_payment_instructions::amount::Amount,
        expected_description_hash: [u8; 32],
    ) -> bitcoin_payment_instructions::hrn_resolution::LNURLResolutionFuture<'a> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Test vectors from https://lndetective.dev/#examples and BOLT specs

    // BIP-21 payment URIs (from lndetective.dev)
    const BITCOIN_ADDRESS: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    const BIP21_SIMPLE: &str = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
    const BIP21_WITH_AMOUNT: &str = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50.0";
    const BIP21_WITH_LABEL: &str = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?label=Luke-Jr";
    const BIP21_WITH_MESSAGE: &str =
        "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?message=Donation%20for%20project%20xyz";
    const BIP21_WITH_ALL: &str = "bitcoin:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa?amount=50.0&label=Luke-Jr&message=Donation%20for%20project%20xyz";

    // BOLT-12 offer that works with the library (from bitcoin-payment-instructions tests)
    // This offer is for signet and has an amount
    const BOLT12_OFFER_SIGNET: &str = "lno1qgs0v8hw8d368q9yw7sx8tejk2aujlyll8cp7tzzyh5h8xyppqqqqqqgqvqcdgq2qenxzatrv46pvggrv64u366d5c0rr2xjc3fq6vw2hh6ce3f9p7z4v4ee0u7avfynjw9q";

    #[tokio::test]
    async fn parse_bitcoin_address() {
        let parsed = PaymentInstruction::parse(BITCOIN_ADDRESS).await.unwrap();
        match parsed {
            PaymentInstruction::BitcoinPaymentInstrunction(inner) => {
                assert!(inner.recipient_description().is_none());
            }
        }
    }

    #[tokio::test]
    async fn parse_bip21_simple() {
        let parsed = PaymentInstruction::parse(BIP21_SIMPLE).await.unwrap();
        match parsed {
            PaymentInstruction::BitcoinPaymentInstrunction(inner) => {
                assert!(inner.recipient_description().is_none());
            }
        }
    }

    #[tokio::test]
    async fn parse_bip21_with_amount() {
        let parsed = PaymentInstruction::parse(BIP21_WITH_AMOUNT).await.unwrap();
        match &parsed {
            PaymentInstruction::BitcoinPaymentInstrunction(inner) => {
                // Amount is 50 BTC = 5_000_000_000 sats
                if let bitcoin_payment_instructions::PaymentInstructions::FixedAmount(fixed) = inner
                {
                    assert_eq!(
                        fixed.onchain_payment_amount(),
                        Some(
                            bitcoin_payment_instructions::amount::Amount::from_sats(5_000_000_000)
                                .unwrap()
                        )
                    );
                } else {
                    panic!("Expected FixedAmount for BIP21 with amount parameter");
                }
            }
        }
    }

    #[tokio::test]
    async fn parse_bip21_with_label() {
        let parsed = PaymentInstruction::parse(BIP21_WITH_LABEL).await.unwrap();
        match parsed {
            PaymentInstruction::BitcoinPaymentInstrunction(_inner) => {
                // Label is parsed but not exposed as description in this crate
            }
        }
    }

    #[tokio::test]
    async fn parse_bip21_with_message() {
        let parsed = PaymentInstruction::parse(BIP21_WITH_MESSAGE).await.unwrap();
        match parsed {
            PaymentInstruction::BitcoinPaymentInstrunction(_inner) => {
                // Message is parsed
            }
        }
    }

    #[tokio::test]
    async fn parse_bip21_with_all_params() {
        let parsed = PaymentInstruction::parse(BIP21_WITH_ALL).await.unwrap();
        match &parsed {
            PaymentInstruction::BitcoinPaymentInstrunction(inner) => {
                if let bitcoin_payment_instructions::PaymentInstructions::FixedAmount(fixed) = inner
                {
                    assert_eq!(
                        fixed.onchain_payment_amount(),
                        Some(
                            bitcoin_payment_instructions::amount::Amount::from_sats(5_000_000_000)
                                .unwrap()
                        )
                    );
                } else {
                    panic!("Expected FixedAmount");
                }
            }
        }
    }

    #[tokio::test]
    async fn parse_bolt12_offer() {
        // Use the tested offer from bitcoin-payment-instructions library
        let hrn_resolver = NoOpHrnResolver;
        let network = Network::Signet;
        let result = bitcoin_payment_instructions::PaymentInstructions::parse(
            BOLT12_OFFER_SIGNET,
            network,
            &hrn_resolver,
            false,
        )
        .await;
        assert!(
            result.is_ok(),
            "Failed to parse BOLT12 offer: {:?}",
            result.err()
        );

        // Verify it has the expected amount and description
        if let Ok(bitcoin_payment_instructions::PaymentInstructions::FixedAmount(fixed)) = &result {
            assert!(fixed.ln_payment_amount().is_some());
            assert_eq!(fixed.recipient_description(), Some("faucet"));
        } else {
            panic!("Expected FixedAmount for BOLT12 offer with amount");
        }
    }

    #[tokio::test]
    async fn parse_bolt12_offer_via_bip21() {
        // BOLT-12 offers can also be embedded in BIP-21 URIs via lno= parameter
        let hrn_resolver = NoOpHrnResolver;
        let network = Network::Signet;

        let bip21_with_offer = format!("bitcoin:?lno={}", BOLT12_OFFER_SIGNET);
        let result = bitcoin_payment_instructions::PaymentInstructions::parse(
            &bip21_with_offer,
            network,
            &hrn_resolver,
            false,
        )
        .await;
        assert!(
            result.is_ok(),
            "Failed to parse BIP21 with BOLT12 offer: {:?}",
            result.err()
        );
    }

    // Test case-insensitive URI scheme (only the scheme, not the address)
    #[tokio::test]
    async fn parse_bip21_uppercase_scheme() {
        // The bitcoin: scheme is case-insensitive, but the address is case-sensitive
        // So we uppercase only the scheme part
        let upper_scheme = "BITCOIN:1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";
        let parsed = PaymentInstruction::parse(upper_scheme).await.unwrap();
        match parsed {
            PaymentInstruction::BitcoinPaymentInstrunction(_inner) => {
                // Successfully parsed uppercase URI scheme
            }
        }
    }
}
