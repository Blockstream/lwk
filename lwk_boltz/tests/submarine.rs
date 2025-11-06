mod utils;

#[cfg(test)]
mod tests {

    use crate::utils::{self, BOLTZ_REGTEST, DEFAULT_REGTEST_NODE, TIMEOUT, WAIT_TIME};
    use std::{env, str::FromStr, sync::Arc, time::Duration};

    use bip39::Mnemonic;
    use boltz_client::{
        boltz::{BoltzApiClientV2, BoltzWsConfig, CreateSubmarineRequest},
        fees::Fee,
        network::{Chain, LiquidChain},
        swaps::{ChainClient, SwapScript, SwapTransactionParams},
        util::sleep,
        Keypair, PublicKey, Secp256k1,
    };
    use lwk_boltz::{
        clients::{AnyClient, ElectrumClient},
        BoltzSession, LightningPayment, PreparePayDataSerializable,
    };
    use lwk_wollet::{elements, secp256k1::rand::thread_rng, ElementsNetwork};

    #[tokio::test]
    #[ignore = "mainnet"]
    async fn test_session_submarine_mainnet() {
        let _ = env_logger::try_init();

        let network = ElementsNetwork::Liquid;

        let session = BoltzSession::builder(
            network,
            AnyClient::Electrum(Arc::new(
                ElectrumClient::new(
                    "elements-mainnet.blockstream.info:50002",
                    true,
                    true,
                    network,
                )
                .unwrap(),
            )),
        )
        .create_swap_timeout(TIMEOUT)
        .build()
        .await
        .unwrap();

        // In a real mainnet test, you would need to provide an actual Lightning invoice
        // This is a placeholder - in practice you'd need to generate this externally
        let bolt11_invoice = env::var("MAINNET_INVOICE")
            .expect("MAINNET_INVOICE environment variable must be set for mainnet submarine test");
        let refund_address = env::var("MAINNET_REFUND_ADDRESS").expect(
            "MAINNET_REFUND_ADDRESS environment variable must be set for mainnet submarine test",
        );

        log::info!("Preparing payment for invoice: {}", bolt11_invoice);

        let refund_address = elements::Address::from_str(&refund_address).unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();

        let prepare_pay_response = session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();
        log::info!(
            "Send {} sats to address: {}",
            prepare_pay_response
                .data
                .create_swap_response
                .expected_amount,
            prepare_pay_response.data.create_swap_response.address
        );
        log::info!("Waiting for payment to be sent to the address...");

        // Note: In a real test, you would need to send funds to prepare_pay_response.address
        // with amount prepare_pay_response.amount before calling complete_pay()
        let result = prepare_pay_response.complete_pay().await;
        log::info!("Complete Pay Result: {:?}", result);
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_submarine() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let mining_handle = utils::start_block_mining();

        let refund_address = utils::generate_address(Chain::Liquid(LiquidChain::LiquidRegtest))
            .await
            .unwrap();
        let refund_address = elements::Address::from_str(&refund_address).unwrap();
        let client = Arc::new(
            ElectrumClient::new(
                DEFAULT_REGTEST_NODE,
                false,
                false,
                ElementsNetwork::default_regtest(),
            )
            .unwrap(),
        );

        let session = BoltzSession::builder(
            ElementsNetwork::default_regtest(),
            AnyClient::Electrum(client.clone()),
        )
        .create_swap_timeout(TIMEOUT)
        .build()
        .await
        .unwrap();
        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();
        let prepare_pay_response = session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();
        utils::send_to_address(
            Chain::Liquid(LiquidChain::LiquidRegtest),
            &prepare_pay_response.data.create_swap_response.address,
            prepare_pay_response
                .data
                .create_swap_response
                .expected_amount,
        )
        .await
        .unwrap();
        prepare_pay_response.complete_pay().await.unwrap();

        // Test underpay which triggers a refund to the refund address
        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();
        let prepare_pay_response = session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();
        utils::send_to_address(
            Chain::Liquid(LiquidChain::LiquidRegtest),
            &prepare_pay_response.data.create_swap_response.address,
            prepare_pay_response
                .data
                .create_swap_response
                .expected_amount
                - 1, // underpay to trigger refund
        )
        .await
        .unwrap();
        prepare_pay_response.complete_pay().await.unwrap();

        // test polling
        let session_polling = BoltzSession::builder(
            ElementsNetwork::default_regtest(),
            AnyClient::Electrum(client.clone()),
        )
        .polling(true)
        .build()
        .await
        .unwrap();

        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();
        let mut prepare_pay_response = session_polling
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();
        utils::send_to_address(
            Chain::Liquid(LiquidChain::LiquidRegtest),
            &prepare_pay_response.data.create_swap_response.address,
            prepare_pay_response
                .data
                .create_swap_response
                .expected_amount,
        )
        .await
        .unwrap();

        // Poll for updates until payment is complete
        loop {
            match prepare_pay_response.advance().await {
                Ok(std::ops::ControlFlow::Continue(update)) => {
                    log::info!("Polling: Received update. status:{}", update.status);
                }
                Ok(std::ops::ControlFlow::Break(result)) => {
                    log::info!("Polling: Payment completed with result: {}", result);
                    assert!(result, "Payment should succeed");
                    break;
                }
                Err(lwk_boltz::Error::NoUpdate) => {
                    log::info!("Polling: No update available, sleeping and retrying...");
                    sleep(Duration::from_secs(1)).await;
                }
                Err(e) => {
                    panic!("Polling: Unexpected error: {}", e);
                }
            }
        }

        // Stop the mining task
        mining_handle.abort();
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_restore_submarine() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let mining_handle = utils::start_block_mining();
        let mnemonic = Mnemonic::from_str(
            "damp cart merit asset obvious idea chef traffic absent armed road link",
        )
        .unwrap();

        let refund_address = utils::generate_address(Chain::Liquid(LiquidChain::LiquidRegtest))
            .await
            .unwrap();
        let refund_address = elements::Address::from_str(&refund_address).unwrap();
        let client = Arc::new(
            ElectrumClient::new(
                DEFAULT_REGTEST_NODE,
                false,
                false,
                ElementsNetwork::default_regtest(),
            )
            .unwrap(),
        );

        let session = BoltzSession::builder(
            ElementsNetwork::default_regtest(),
            AnyClient::Electrum(client.clone()),
        )
        .create_swap_timeout(TIMEOUT)
        .mnemonic(mnemonic.clone())
        .build()
        .await
        .unwrap();

        // test restore swap after drop
        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();
        let prepare_pay_response = session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();

        let serialized_data = prepare_pay_response.serialize().unwrap();
        drop(prepare_pay_response);
        drop(session);
        let session = BoltzSession::builder(
            ElementsNetwork::default_regtest(),
            AnyClient::Electrum(client.clone()),
        )
        .create_swap_timeout(TIMEOUT)
        .mnemonic(mnemonic)
        .build()
        .await
        .unwrap();
        let data = PreparePayDataSerializable::deserialize(&serialized_data).unwrap();
        assert_eq!(
            data.mnemonic_identifier.to_string(),
            "e92cd0870c080a91a063345362b7e76d4ad3a4b4"
        );
        let prepare_pay_response = session.restore_prepare_pay(data).await.unwrap();
        utils::send_to_address(
            Chain::Liquid(LiquidChain::LiquidRegtest),
            &prepare_pay_response.address(),
            prepare_pay_response.amount(),
        )
        .await
        .unwrap();
        prepare_pay_response.complete_pay().await.unwrap();

        // Stop the mining task
        mining_handle.abort();
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_submarine() {
        let _ = env_logger::try_init();
        let chain_client = ChainClient::new().with_liquid(
            ElectrumClient::new(
                DEFAULT_REGTEST_NODE,
                false,
                false,
                ElementsNetwork::default_regtest(),
            )
            .unwrap(),
        );
        let chain = Chain::Liquid(LiquidChain::LiquidRegtest);
        v2_submarine(&chain_client, false, chain).await;
        v2_submarine(&chain_client, true, chain).await;
    }

    async fn v2_submarine(chain_client: &ChainClient, underpay: bool, chain: Chain) {
        let secp = Secp256k1::new();
        let our_keys = Keypair::new(&secp, &mut thread_rng());

        let refund_public_key = PublicKey {
            inner: our_keys.public_key(),
            compressed: true,
        };

        // Set a new invoice string and refund address for each test.
        let invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let refund_address = utils::generate_address(chain).await.unwrap();

        let boltz_api_v2 = BoltzApiClientV2::new(BOLTZ_REGTEST.to_string(), Some(TIMEOUT));
        let ws_api = Arc::new(boltz_api_v2.ws(BoltzWsConfig::default()));
        utils::start_ws(ws_api.clone());

        // If there is MRH send directly to that address
        //    let (bip21_addrs, amount) =
        //         check_for_mrh(&boltz_api_v2, &invoice, chain).unwrap();
        //         log::info!("Found MRH in invoice");
        //         log::info!("Send {} to {}", amount, bip21_addrs);
        //         return;

        // Initiate the swap with Boltz
        let create_swap_req = CreateSubmarineRequest {
            from: chain.to_string(),
            to: "BTC".to_string(),
            invoice: invoice.to_string(),
            refund_public_key,
            pair_hash: None,
            referral_id: None,
            webhook: None,
        };

        let create_swap_response = boltz_api_v2.post_swap_req(&create_swap_req).await.unwrap();

        log::info!("Got Swap Response from Boltz server");

        create_swap_response
            .validate(&invoice, &refund_public_key, chain)
            .unwrap();
        log::info!("VALIDATED RESPONSE!");

        log::debug!("Swap Response: {create_swap_response:?}");

        let swap_script =
            SwapScript::submarine_from_swap_resp(chain, &create_swap_response, refund_public_key)
                .unwrap();
        let swap_id = create_swap_response.id.clone();
        log::debug!("Created Swap Script. : {swap_script:?}");

        let mut rx = ws_api.updates();
        ws_api.subscribe_swap(&swap_id).await.unwrap();
        // Event handlers for various swap status.
        loop {
            let update = rx.recv().await.unwrap();
            match update.status.as_str() {
                "invoice.set" => {
                    log::info!(
                        "Send {} sats to {} address {}",
                        create_swap_response.expected_amount,
                        chain,
                        create_swap_response.address
                    );

                    let amount = match underpay {
                        true => create_swap_response.expected_amount - 1,
                        false => create_swap_response.expected_amount,
                    };
                    utils::send_to_address(chain, &create_swap_response.address, amount)
                        .await
                        .unwrap();
                }
                "transaction.mempool" => {
                    utils::mine_blocks(1).await.unwrap();
                }
                "transaction.claim.pending" => {
                    let response = swap_script
                        .submarine_cooperative_claim(
                            &swap_id,
                            &our_keys,
                            &create_swap_req.invoice,
                            &boltz_api_v2,
                        )
                        .await
                        .unwrap();
                    log::debug!("Received claim tx details : {response:?}");
                }

                "transaction.claimed" => {
                    log::info!("Successfully completed submarine swap");
                    break;
                }

                // This means the funding transaction was rejected by Boltz for whatever reason, and we need to get
                // the funds back via refund.
                "transaction.lockupFailed" | "invoice.failedToPay" => {
                    sleep(WAIT_TIME).await;
                    let tx = swap_script
                        .construct_refund(SwapTransactionParams {
                            keys: our_keys,
                            output_address: refund_address,
                            fee: Fee::Absolute(1000),
                            swap_id: swap_id.clone(),
                            chain_client,
                            boltz_client: &boltz_api_v2,
                            options: None,
                        })
                        .await
                        .unwrap();

                    let txid = chain_client.broadcast_tx(&tx).await.unwrap();
                    log::info!("Cooperative Refund Successfully broadcasted: {txid}");

                    // Non cooperative refund requires expired swap
                    /*log::info!("Cooperative refund failed. {:?}", e);
                    log::info!("Attempting Non-cooperative refund.");

                    let tx = swap_tx
                        .sign_refund(&our_keys, Fee::Absolute(1000), None)
                        .await
                        .unwrap();
                    let txid = swap_tx
                        .broadcast(&tx, bitcoin_client)
                        .await
                        .unwrap();
                    log::info!("Non-cooperative Refund Successfully broadcasted: {}", txid);*/
                    break;
                }
                _ => {
                    log::info!("Got Update from server: {}", update.status);
                }
            };
        }
    }
}
