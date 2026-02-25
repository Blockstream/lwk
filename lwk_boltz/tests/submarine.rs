mod utils;

#[cfg(test)]
mod tests {

    use crate::utils::{self, BOLTZ_REGTEST, DEFAULT_REGTEST_NODE, TIMEOUT, WAIT_TIME};
    use std::{str::FromStr, sync::Arc, time::Duration};

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
        BoltzSession, LightningPayment, PreparePayDataSerializable, SwapPersistence,
    };
    use lwk_wollet::{elements, secp256k1::rand::thread_rng, ElementsNetwork};

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

        // complete a payment via advance()
        let bolt11_invoice = utils::generate_invoice_lnd(500_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();
        let mut prepare_pay_response = session
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
        loop {
            match prepare_pay_response.advance().await {
                Ok(std::ops::ControlFlow::Continue(_)) => {}
                Ok(std::ops::ControlFlow::Break(result)) => {
                    log::info!("Payment completed with result: {result}");
                    assert!(result, "Payment should succeed");
                    break;
                }
                Err(e) => {
                    panic!("Unexpected error: {e}");
                }
            }
        }
        assert!(
            prepare_pay_response.claim_txid().is_some(),
            "claim txid should be available when submarine swap is claimed"
        );
        // repeatly calling advance on a terminated swap don't timeout
        for _ in 0..10 {
            match prepare_pay_response.advance().await {
                Err(lwk_boltz::Error::NoBoltzUpdate) => { // expected
                }
                _ => {
                    panic!("unexpected status");
                }
            }
        }

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
                    log::info!("Polling: Payment completed with result: {result}");
                    assert!(result, "Payment should succeed");
                    break;
                }
                Err(lwk_boltz::Error::NoBoltzUpdate) => {
                    log::info!("Polling: No update available, sleeping and retrying...");
                    sleep(Duration::from_secs(1)).await;
                }
                Err(e) => {
                    panic!("Polling: Unexpected error: {e}");
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
            &prepare_pay_response.uri_address().unwrap().to_string(),
            prepare_pay_response.uri_amount(),
        )
        .await
        .unwrap();
        prepare_pay_response.complete_pay().await.unwrap();

        // Stop the mining task
        mining_handle.abort();
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_restore_claim_txid_submarine() {
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

        let session_fn = || {
            BoltzSession::builder(
                ElementsNetwork::default_regtest(),
                AnyClient::Electrum(client.clone()),
            )
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic.clone())
            .build()
        };

        let session = session_fn().await.unwrap();
        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();
        let mut prepare_pay_response = session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();

        let serialized_data = prepare_pay_response.serialize().unwrap();
        let data = PreparePayDataSerializable::deserialize(&serialized_data).unwrap();
        assert_eq!(data.claim_txid.as_deref(), None);

        utils::send_to_address(
            Chain::Liquid(LiquidChain::LiquidRegtest),
            &prepare_pay_response.uri_address().unwrap().to_string(),
            prepare_pay_response.uri_amount(),
        )
        .await
        .unwrap();

        while let Ok(std::ops::ControlFlow::Continue(_)) = prepare_pay_response.advance().await {}

        let claim_txid = prepare_pay_response
            .claim_txid()
            .map(|s| s.to_string())
            .expect("claim_txid should be set");
        log::info!("claim_txid: {claim_txid}");
        drop(prepare_pay_response);
        drop(session);

        let session = session_fn().await.unwrap();
        let prepare_pay_response = session.restore_prepare_pay(data).await.unwrap();
        let claim_txid_restored = prepare_pay_response
            .claim_txid()
            .expect("claim_txid should be set");
        assert_eq!(claim_txid, claim_txid_restored);

        // Stop the mining task
        mining_handle.abort();
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_submarine_duplicate_invoice_error() {
        let _ = env_logger::try_init();

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
            AnyClient::Electrum(client),
        )
        .create_swap_timeout(TIMEOUT)
        .build()
        .await
        .unwrap();

        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();

        session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();

        let err = session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap_err();
        assert!(err
            .to_string()
            .contains("a swap with this invoice exists already"));
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_restore_submarine_from_swap_list() {
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

        // Create a swap but don't complete it
        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();
        let prepare_pay_response = session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();

        let swap_id = prepare_pay_response.swap_id();
        let swap_list = session.swap_restore().await.unwrap();
        let restorable = session
            .restorable_submarine_swaps(&swap_list, &refund_address)
            .await
            .unwrap();
        let swaps: Vec<_> = restorable
            .iter()
            .filter(|data| data.create_swap_response.id == swap_id)
            .collect();
        log::info!("Found {swaps:?} restorable submarine swaps");
        assert_eq!(swaps.len(), 0); // the just created swap is not restorable.

        utils::send_to_address(
            Chain::Liquid(LiquidChain::LiquidRegtest),
            &prepare_pay_response.uri_address().unwrap().to_string(),
            prepare_pay_response.uri_amount(),
        )
        .await
        .unwrap();
        sleep(Duration::from_secs(3)).await;
        let swap_id = prepare_pay_response.swap_id().to_string();

        // Drop the response and session (simulating app crash/restart without serializing)
        drop(prepare_pay_response);
        drop(session);

        // Create a new session with the same mnemonic
        let session = BoltzSession::builder(
            ElementsNetwork::default_regtest(),
            AnyClient::Electrum(client.clone()),
        )
        .create_swap_timeout(TIMEOUT)
        .mnemonic(mnemonic)
        .build()
        .await
        .unwrap();

        // Get all swaps from Boltz API
        let swap_list = session.swap_restore().await.unwrap();
        log::info!("Found {} swaps in swap_restore", swap_list.len());

        // Filter to get restorable submarine swaps
        let restorable = session
            .restorable_submarine_swaps(&swap_list, &refund_address)
            .await
            .unwrap();
        log::info!("Found {} restorable submarine swaps", restorable.len());

        // Find our swap in the restorable list
        let our_swap: PreparePayDataSerializable = restorable
            .into_iter()
            .map(|data| data.into())
            .find(|data: &PreparePayDataSerializable| data.create_swap_response.id == swap_id)
            .expect("Our swap should be in the restorable list");

        // Restore and complete the swap
        let prepare_pay_response = session.restore_prepare_pay(our_swap).await.unwrap();
        // Use the captured expected_amount since uri_amount() returns 0 for restored swaps
        // (see WORKAROUND comment above)

        prepare_pay_response.complete_pay().await.unwrap();
        log::info!("Swap completed successfully");

        // Stop the mining task
        mining_handle.abort();
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_restore_submarine_with_store() {
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

        // Create a shared store that persists across sessions
        let store = Arc::new(lwk_common::MemoryStore::new());

        let session = BoltzSession::builder(
            ElementsNetwork::default_regtest(),
            AnyClient::Electrum(client.clone()),
        )
        .create_swap_timeout(TIMEOUT)
        .mnemonic(mnemonic.clone())
        .store(store.clone())
        .build()
        .await
        .unwrap();

        // Initially no pending swaps
        let pending = session.pending_swap_ids().unwrap();
        assert!(pending.is_empty(), "Should start with no pending swaps");

        // Create a second session with a DIFFERENT mnemonic but the SAME store
        // This tests that encrypted keys don't collide between sessions
        let mnemonic2 = Mnemonic::from_str(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .unwrap();
        let session2 = BoltzSession::builder(
            ElementsNetwork::default_regtest(),
            AnyClient::Electrum(client.clone()),
        )
        .create_swap_timeout(TIMEOUT)
        .mnemonic(mnemonic2.clone())
        .store(store.clone())
        .build()
        .await
        .unwrap();

        // Session 2 should also see no pending swaps (different mnemonic = different encrypted keys)
        let pending2 = session2.pending_swap_ids().unwrap();
        assert!(
            pending2.is_empty(),
            "Session 2 should start with no pending swaps"
        );

        // Create a swap - it should be automatically persisted
        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        let lightning_payment = LightningPayment::from_str(&bolt11_invoice).unwrap();
        let prepare_pay_response = session
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .unwrap();

        let swap_id = prepare_pay_response.swap_id().to_string();

        // Verify swap is in pending list
        let pending = session.pending_swap_ids().unwrap();
        assert!(
            pending.contains(&swap_id),
            "Swap should be in pending list after creation"
        );

        // Verify swap data is stored
        let swap_data = session.get_swap_data(&swap_id).unwrap();
        assert!(swap_data.is_some(), "Swap data should be stored");

        // IMPORTANT: Verify session2 cannot see session1's swap (key isolation test)
        let pending2 = session2.pending_swap_ids().unwrap();
        assert!(
            !pending2.contains(&swap_id),
            "Session 2 should NOT see session 1's swap - keys should be isolated"
        );
        assert!(
            session2.get_swap_data(&swap_id).unwrap().is_none(),
            "Session 2 should NOT be able to read session 1's swap data"
        );

        // Drop the sessions and response
        drop(prepare_pay_response);
        drop(session);
        drop(session2);

        // Create a new session with the same store
        let session = BoltzSession::builder(
            ElementsNetwork::default_regtest(),
            AnyClient::Electrum(client.clone()),
        )
        .create_swap_timeout(TIMEOUT)
        .mnemonic(mnemonic)
        .store(store.clone())
        .build()
        .await
        .unwrap();

        // Verify swap is still in pending list
        let pending = session.pending_swap_ids().unwrap();
        assert!(
            pending.contains(&swap_id),
            "Swap should still be in pending list after session restart"
        );

        // Restore the swap from store data
        let swap_data_json = session.get_swap_data(&swap_id).unwrap().unwrap();
        let data = PreparePayDataSerializable::deserialize(&swap_data_json).unwrap();
        let prepare_pay_response = session.restore_prepare_pay(data).await.unwrap();

        // Send funds and complete the swap
        utils::send_to_address(
            Chain::Liquid(LiquidChain::LiquidRegtest),
            &prepare_pay_response.uri_address().unwrap().to_string(),
            prepare_pay_response.uri_amount(),
        )
        .await
        .unwrap();
        prepare_pay_response.complete_pay().await.unwrap();

        // Verify swap moved from pending to completed
        let pending = session.pending_swap_ids().unwrap();
        let completed = session.completed_swap_ids().unwrap();
        assert!(
            !pending.contains(&swap_id),
            "Swap should not be in pending list after completion"
        );
        assert!(
            completed.contains(&swap_id),
            "Swap should be in completed list after completion"
        );

        // Test remove_swap
        session.remove_swap(&swap_id).unwrap();
        let pending = session.pending_swap_ids().unwrap();
        let completed = session.completed_swap_ids().unwrap();
        assert!(
            !pending.contains(&swap_id),
            "Swap should be removed from pending"
        );
        assert!(
            !completed.contains(&swap_id),
            "Swap should be removed from completed"
        );
        assert!(
            session.get_swap_data(&swap_id).unwrap().is_none(),
            "Swap data should be removed"
        );

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
