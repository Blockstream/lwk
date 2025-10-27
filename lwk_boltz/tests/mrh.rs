mod utils;

#[cfg(test)]
mod tests {
    use crate::utils::{self, DEFAULT_REGTEST_NODE, TIMEOUT};
    use std::{str::FromStr, sync::Arc};

    use boltz_client::{
        boltz::BoltzApiClientV2,
        network::{Chain, LiquidChain},
        swaps::magic_routing::check_for_mrh,
    };
    use lwk_boltz::{
        clients::{AnyClient, ElectrumClient},
        Error, LightningSession,
    };
    use lwk_wollet::{elements, ElementsNetwork};

    /// Test Magic Routing Hints: A Boltz wallet pays another Boltz wallet's invoice
    /// directly on-chain without performing a swap
    #[tokio::test]
    async fn test_session_mrh() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let _mining_handle = utils::start_block_mining();
        let network = ElementsNetwork::default_regtest();
        let client =
            Arc::new(ElectrumClient::new(DEFAULT_REGTEST_NODE, false, false, network).unwrap());

        // Receiver: Create a LightningSession and generate an invoice with MRH
        let receiver_session = LightningSession::new(
            network,
            AnyClient::Electrum(client.clone()),
            Some(TIMEOUT),
            None,
        )
        .await
        .unwrap();
        let claim_address = utils::generate_address(Chain::Liquid(LiquidChain::LiquidRegtest))
            .await
            .unwrap();
        let claim_address = elements::Address::from_str(&claim_address).unwrap();

        let invoice_amount = 100_000;
        let invoice = receiver_session
            .invoice(
                invoice_amount,
                Some("MRH test".to_string()),
                &claim_address,
                None,
            )
            .await
            .unwrap();
        log::info!("claim_address: {}", claim_address);
        log::info!("Receiver created invoice: {}", invoice.bolt11_invoice());
        log::info!("Invoice fee: {:?}", invoice.data.fee);

        // Check for magic routing hint
        let boltz_api = BoltzApiClientV2::new(utils::BOLTZ_REGTEST.to_string(), Some(TIMEOUT));
        let mrh_result = check_for_mrh(
            &boltz_api,
            &invoice.bolt11_invoice().to_string(),
            Chain::Liquid(LiquidChain::LiquidRegtest),
        )
        .await
        .unwrap();

        assert!(
            mrh_result.is_some(),
            "Magic routing hint should be present in the invoice"
        );

        let (mrh_address, mrh_amount) = mrh_result.unwrap();
        log::info!(
            "Found MRH - Address: {}, Amount: {}",
            mrh_address,
            mrh_amount
        );

        // Verify the MRH amount is less than the invoice amount (due to fees)
        assert!(
            mrh_amount.to_sat() < invoice_amount,
            "MRH amount {} should be less than invoice amount {} due to fees",
            mrh_amount.to_sat(),
            invoice_amount
        );

        // The difference should be reasonable (the fee)
        let fee_diff = invoice_amount - mrh_amount.to_sat();
        log::info!(
            "Fee difference: {} sats (invoice.fee was {:?})",
            fee_diff,
            invoice.data.fee
        );
        assert!(
            fee_diff > 0 && fee_diff < invoice_amount / 10,
            "Fee should be positive and reasonable"
        );

        // TODO complete the payment from a sender that detects the MRH and pays directly to the MRH address

        // Sender: Detect MRH in the invoice
        let sender_session = LightningSession::new(
            network,
            AnyClient::Electrum(client.clone()),
            Some(TIMEOUT),
            None,
        )
        .await
        .unwrap();
        let bolt11_parsed = invoice.bolt11_invoice();
        let prepare_pay_response = sender_session
            .prepare_pay(&bolt11_parsed, &claim_address, None)
            .await;
        if let Err(Error::MagicRoutingHint {
            address,
            amount,
            uri,
        }) = prepare_pay_response
        {
            utils::send_to_address(Chain::Liquid(LiquidChain::LiquidRegtest), &address, amount)
                .await
                .unwrap();
            log::info!("Sent {} sats to {} or use uri {}", amount, address, uri);
        }

        invoice.complete_pay().await.unwrap();
    }
}
