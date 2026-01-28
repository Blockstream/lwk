mod utils;

#[cfg(test)]
mod tests {
    use crate::utils::{self, TIMEOUT, WAIT_TIME};
    use crate::utils::{next_status, DEFAULT_REGTEST_NODE};
    use bip39::Mnemonic;
    use boltz_client::boltz::BoltzApiClientV2;
    use boltz_client::boltz::BoltzWsConfig;
    use boltz_client::boltz::CreateChainRequest;
    use boltz_client::boltz::Side;
    use boltz_client::boltz::BOLTZ_REGTEST;
    use boltz_client::fees::Fee;
    use boltz_client::network::electrum::ElectrumBitcoinClient;
    use boltz_client::network::{BitcoinChain, Chain, LiquidChain};
    use boltz_client::swaps::ChainClient;
    use boltz_client::swaps::SwapScript;
    use boltz_client::swaps::{SwapTransactionParams, TransactionOptions};
    use boltz_client::util::{secrets::Preimage, sleep};
    use boltz_client::Keypair;
    use boltz_client::PublicKey;
    use boltz_client::Secp256k1;
    use lwk_boltz::SwapPersistence;
    use lwk_boltz::{
        clients::{AnyClient, ElectrumClient},
        BoltzSession, SwapAsset, LIQUID_UNCOOPERATIVE_EXTRA,
    };
    use lwk_wollet::bitcoin;
    use lwk_wollet::elements;
    use lwk_wollet::secp256k1::rand::thread_rng;
    use lwk_wollet::ElementsNetwork;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::time::Duration;

    const BTC_CHAIN: BitcoinChain = BitcoinChain::BitcoinRegtest;
    const LBTC_CHAIN: LiquidChain = LiquidChain::LiquidRegtest;

    pub fn create_chain_client_electrum() -> ChainClient {
        let liquid_client = ElectrumClient::new(
            DEFAULT_REGTEST_NODE,
            false,
            false,
            ElementsNetwork::default_regtest(),
        )
        .unwrap();
        ChainClient::new()
            .with_bitcoin(ElectrumBitcoinClient::default(BTC_CHAIN, None).unwrap())
            .with_liquid(liquid_client)
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_chain_swaps_btc_lbtc() {
        let chain_client = create_chain_client_electrum();
        v2_chain(&chain_client, false, BTC_CHAIN.into(), LBTC_CHAIN.into()).await;
        v2_chain(&chain_client, true, BTC_CHAIN.into(), LBTC_CHAIN.into()).await;
    }
    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_chain_swaps_lbtc_btc() {
        let chain_client = create_chain_client_electrum();
        v2_chain(&chain_client, false, LBTC_CHAIN.into(), BTC_CHAIN.into()).await;
        v2_chain(&chain_client, true, LBTC_CHAIN.into(), BTC_CHAIN.into()).await;
    }

    async fn v2_chain(chain_client: &ChainClient, underpay: bool, from: Chain, to: Chain) {
        let _ = env_logger::try_init();

        let secp = Secp256k1::new();
        let preimage = Preimage::random();
        log::info!("{preimage:#?}");
        let our_claim_keys = Keypair::new(&secp, &mut thread_rng());
        let claim_public_key = PublicKey {
            compressed: true,
            inner: our_claim_keys.public_key(),
        };

        let our_refund_keys = Keypair::new(&secp, &mut thread_rng());
        log::info!("Refund: {:#?}", our_refund_keys.display_secret());

        let refund_public_key = PublicKey {
            inner: our_refund_keys.public_key(),
            compressed: true,
        };

        let create_chain_req = CreateChainRequest {
            from: from.to_string(),
            to: to.to_string(),
            preimage_hash: preimage.sha256,
            claim_public_key: Some(claim_public_key),
            refund_public_key: Some(refund_public_key),
            referral_id: None,
            user_lock_amount: Some(50_000),
            server_lock_amount: None,
            pair_hash: None,
            webhook: None,
        };

        let boltz_api_v2 = BoltzApiClientV2::new(BOLTZ_REGTEST.to_string(), Some(TIMEOUT));

        let create_chain_response = boltz_api_v2.post_chain_req(create_chain_req).await.unwrap();
        create_chain_response
            .validate(&claim_public_key, &refund_public_key, from, to)
            .unwrap();
        let swap_id = create_chain_response.clone().id;
        let lockup_details = create_chain_response.clone().lockup_details;

        let lockup_script = SwapScript::chain_from_swap_resp(
            from,
            Side::Lockup,
            lockup_details.clone(),
            refund_public_key,
        )
        .unwrap();
        log::debug!("Lockup Script: {lockup_script:#?}");

        let refund_address = utils::generate_address(from).await.unwrap();

        let claim_details = create_chain_response.claim_details;
        let claim_script = SwapScript::chain_from_swap_resp(
            to,
            Side::Claim,
            claim_details.clone(),
            claim_public_key,
        )
        .unwrap();

        let claim_address = utils::generate_address(to).await.unwrap();
        log::debug!("{claim_address:#?}");

        let ws_api = Arc::new(boltz_api_v2.ws(BoltzWsConfig::default()));
        utils::start_ws(ws_api.clone());
        let mut rx = ws_api.updates();
        ws_api.subscribe_swap(&swap_id).await.unwrap();

        log::info!("Subscribed to swap {swap_id}");

        next_status(&mut rx, "swap.created").await.unwrap();

        let amount = match underpay {
            true => create_chain_response.lockup_details.amount / 2,
            false => create_chain_response.lockup_details.amount,
        };
        let address = create_chain_response.lockup_details.clone().lockup_address;

        log::info!("Sending {amount} sats to {from} address {address}");

        utils::send_to_address(from, &address, amount)
            .await
            .unwrap();

        if underpay {
            next_status(&mut rx, "transaction.lockupFailed")
                .await
                .unwrap();

            sleep(WAIT_TIME).await;
            log::info!("REFUNDING!");
            refund_v2_chain(
                lockup_script.clone(),
                refund_address.clone(),
                swap_id.clone(),
                our_refund_keys,
                boltz_api_v2.clone(),
                100,
                chain_client,
            )
            .await;
            if let Chain::Bitcoin(_) = from {
                log::info!("REFUNDING with higher fee");
                refund_v2_chain(
                    lockup_script.clone(),
                    refund_address.clone(),
                    swap_id.clone(),
                    our_refund_keys,
                    boltz_api_v2.clone(),
                    1000,
                    chain_client,
                )
                .await;
            }
        } else {
            next_status(&mut rx, "transaction.mempool").await.unwrap();
            utils::mine_blocks(1).await.unwrap();

            next_status(&mut rx, "transaction.confirmed").await.unwrap();

            next_status(&mut rx, "transaction.server.mempool")
                .await
                .unwrap();
            utils::mine_blocks(1).await.unwrap();

            next_status(&mut rx, "transaction.server.confirmed")
                .await
                .unwrap();

            log::info!("Server lockup tx is confirmed!");

            sleep(WAIT_TIME).await;
            log::info!("Claiming!");

            let swap_params = SwapTransactionParams {
                keys: our_claim_keys,
                output_address: claim_address.clone(),
                fee: Fee::Absolute(1000),
                swap_id: swap_id.clone(),
                options: Some(
                    TransactionOptions::default()
                        .with_chain_claim(our_refund_keys, lockup_script.clone()),
                ),
                chain_client,
                boltz_client: &boltz_api_v2,
            };

            // Constructing a chain tx more than once should work
            let _tx = claim_script
                .construct_claim(&preimage, swap_params.clone())
                .await
                .unwrap();
            let tx = claim_script
                .construct_claim(&preimage, swap_params)
                .await
                .unwrap();

            chain_client.broadcast_tx(&tx).await.unwrap();

            log::info!("Successfully broadcasted claim tx!");

            next_status(&mut rx, "transaction.claimed").await.unwrap();
            log::info!("Successfully completed chain swap");
        }
    }

    async fn refund_v2_chain(
        lockup_script: SwapScript,
        refund_address: String,
        swap_id: String,
        our_refund_keys: Keypair,
        boltz_api_v2: BoltzApiClientV2,
        absolute_fees: u64,
        chain_client: &ChainClient,
    ) {
        let tx = lockup_script
            .construct_refund(SwapTransactionParams {
                keys: our_refund_keys,
                output_address: refund_address,
                fee: Fee::Absolute(absolute_fees),
                swap_id: swap_id.clone(),
                chain_client,
                boltz_client: &boltz_api_v2,
                options: None,
            })
            .await
            .unwrap();

        chain_client.broadcast_tx(&tx).await.unwrap();

        log::info!("Successfully broadcasted refund tx!");
        log::debug!("Refund Tx {tx:#?}");
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_restore_chain_swaps() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let mining_handle = crate::utils::start_block_mining();

        let network = ElementsNetwork::default_regtest();
        let client =
            Arc::new(ElectrumClient::new(DEFAULT_REGTEST_NODE, false, false, network).unwrap());

        let mnemonic = Mnemonic::from_str(
            "damp cart merit asset obvious idea chef traffic absent armed road link",
        )
        .unwrap();

        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic.clone())
            .build()
            .await
            .unwrap();

        // Test BTC to LBTC swap with restore
        let refund_address_str = crate::utils::generate_address(BTC_CHAIN.into())
            .await
            .unwrap();
        let claim_address_str = crate::utils::generate_address(LBTC_CHAIN.into())
            .await
            .unwrap();
        let refund_address = bitcoin::Address::from_str(&refund_address_str)
            .unwrap()
            .assume_checked();
        let claim_address = elements::Address::from_str(&claim_address_str).unwrap();

        let response = session
            .btc_to_lbtc(50_000, &refund_address, &claim_address, None)
            .await
            .unwrap();

        // Serialize and drop
        let serialized_data = response.serialize().unwrap();
        let lockup_address = response.lockup_address().to_string();
        let expected_amount = response.expected_amount();
        drop(response);
        drop(session);

        // Restore session and swap
        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic)
            .build()
            .await
            .unwrap();

        let data = lwk_boltz::ChainSwapDataSerializable::deserialize(&serialized_data).unwrap();
        assert!(data.preimage.is_none());
        assert_eq!(
            data.mnemonic_identifier.to_string(),
            "e92cd0870c080a91a063345362b7e76d4ad3a4b4"
        );

        let response = session.restore_lockup(data).await.unwrap();

        log::info!(
            "Restored BTC to LBTC swap - Lockup address: {}",
            response.lockup_address()
        );

        crate::utils::send_to_address(BTC_CHAIN.into(), &lockup_address, expected_amount)
            .await
            .unwrap();

        let success = response.complete().await.unwrap();
        assert!(success, "Restored BTC to LBTC swap should succeed");
        drop(session);

        // Stop the mining task
        mining_handle.abort();
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_restore_chain_swaps_from_swap_list() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let mining_handle = crate::utils::start_block_mining();

        let network = ElementsNetwork::default_regtest();
        let client =
            Arc::new(ElectrumClient::new(DEFAULT_REGTEST_NODE, false, false, network).unwrap());

        let mnemonic = Mnemonic::from_str(
            "damp cart merit asset obvious idea chef traffic absent armed road link",
        )
        .unwrap();

        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic.clone())
            .build()
            .await
            .unwrap();

        // Test BTC to LBTC swap with restore from swap list
        let refund_address_str = crate::utils::generate_address(BTC_CHAIN.into())
            .await
            .unwrap();
        let claim_address_str = crate::utils::generate_address(LBTC_CHAIN.into())
            .await
            .unwrap();
        let refund_address = bitcoin::Address::from_str(&refund_address_str)
            .unwrap()
            .assume_checked();
        let claim_address = elements::Address::from_str(&claim_address_str).unwrap();

        let response = session
            .btc_to_lbtc(50_000, &refund_address, &claim_address, None)
            .await
            .unwrap();

        let swap_list = session.swap_restore().await.unwrap();
        let restorable = session
            .restorable_btc_to_lbtc_swaps(&swap_list, &claim_address, &refund_address)
            .await
            .unwrap();
        let swaps: Vec<_> = restorable
            .iter()
            .filter(|data| data.create_chain_response.id == response.swap_id())
            .collect();
        log::info!("Found {:?} restorable chain swaps", swaps);
        assert_eq!(swaps.len(), 0); // the just created swap is not restorable.

        let swap_id = response.swap_id().to_string();
        let lockup_address = response.lockup_address().to_string();
        let expected_amount = response.expected_amount();

        utils::send_to_address(BTC_CHAIN.into(), &lockup_address, expected_amount)
            .await
            .unwrap();
        utils::mine_blocks(1).await.unwrap();

        // Drop the response and session (simulating app crash/restart without serializing)
        drop(response);
        drop(session);

        // Create a new session with the same mnemonic
        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic)
            .build()
            .await
            .unwrap();

        // Get all swaps from Boltz API
        let swap_list = session.swap_restore().await.unwrap();
        log::info!("Found {} swaps in swap_restore", swap_list.len());

        // Filter to get restorable chain swaps
        let restorable = session
            .restorable_btc_to_lbtc_swaps(&swap_list, &claim_address, &refund_address)
            .await
            .unwrap();
        log::info!("Found {} restorable chain swaps", restorable.len());

        // Find our swap in the restorable list
        let our_swap: lwk_boltz::ChainSwapDataSerializable = restorable
            .into_iter()
            .map(|data| data.into())
            .find(|data: &lwk_boltz::ChainSwapDataSerializable| {
                data.create_chain_response.id == *swap_id
            })
            .expect("Our swap should be in the restorable list");

        // Restore and complete the swap
        let response = session.restore_lockup(our_swap).await.unwrap();

        let success = response.complete().await.unwrap();
        assert!(
            success,
            "Restored BTC to LBTC swap from swap list should succeed"
        );
        log::info!("Chain swap completed successfully");

        // Stop the mining task
        mining_handle.abort();
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_restore_chain_swaps_with_random_preimages() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let mining_handle = crate::utils::start_block_mining();

        let network = ElementsNetwork::default_regtest();
        let client =
            Arc::new(ElectrumClient::new(DEFAULT_REGTEST_NODE, false, false, network).unwrap());

        let mnemonic = Mnemonic::from_str(
            "damp cart merit asset obvious idea chef traffic absent armed road link",
        )
        .unwrap();

        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic.clone())
            .random_preimages(true)
            .build()
            .await
            .unwrap();

        // Test BTC to LBTC swap with restore
        let refund_address_str = crate::utils::generate_address(BTC_CHAIN.into())
            .await
            .unwrap();
        let claim_address_str = crate::utils::generate_address(LBTC_CHAIN.into())
            .await
            .unwrap();
        let refund_address = bitcoin::Address::from_str(&refund_address_str)
            .unwrap()
            .assume_checked();
        let claim_address = elements::Address::from_str(&claim_address_str).unwrap();

        let response = session
            .btc_to_lbtc(50_000, &refund_address, &claim_address, None)
            .await
            .unwrap();

        // Serialize and drop
        let serialized_data = response.serialize().unwrap();
        let lockup_address = response.lockup_address().to_string();
        let expected_amount = response.expected_amount();
        drop(response);
        drop(session);

        // Restore session and swap
        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic)
            .random_preimages(true)
            .build()
            .await
            .unwrap();

        let data = lwk_boltz::ChainSwapDataSerializable::deserialize(&serialized_data).unwrap();
        assert!(data.preimage.is_some());
        assert_eq!(
            data.mnemonic_identifier.to_string(),
            "e92cd0870c080a91a063345362b7e76d4ad3a4b4"
        );

        let response = session.restore_lockup(data).await.unwrap();

        log::info!(
            "Restored BTC to LBTC swap with random preimages - Lockup address: {}",
            response.lockup_address()
        );

        crate::utils::send_to_address(BTC_CHAIN.into(), &lockup_address, expected_amount)
            .await
            .unwrap();

        let success = response.complete().await.unwrap();
        assert!(
            success,
            "Restored BTC to LBTC swap with random preimages should succeed"
        );
        drop(session);

        // Stop the mining task
        mining_handle.abort();
    }

    #[tokio::test]
    #[ignore = "requires regtest environment"]
    async fn test_session_restore_chain_swaps_with_store() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let mining_handle = crate::utils::start_block_mining();

        let network = ElementsNetwork::default_regtest();
        let client =
            Arc::new(ElectrumClient::new(DEFAULT_REGTEST_NODE, false, false, network).unwrap());

        let mnemonic = Mnemonic::from_str(
            "damp cart merit asset obvious idea chef traffic absent armed road link",
        )
        .unwrap();

        // Create a shared store that persists across sessions
        let store = Arc::new(lwk_common::MemoryStore::new());

        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic.clone())
            .store(store.clone())
            .build()
            .await
            .unwrap();

        // Initially no pending swaps
        let pending = session.pending_swap_ids().unwrap().unwrap();
        assert!(pending.is_empty(), "Should start with no pending swaps");

        // Test BTC to LBTC swap with store-based persistence
        let refund_address_str = crate::utils::generate_address(BTC_CHAIN.into())
            .await
            .unwrap();
        let claim_address_str = crate::utils::generate_address(LBTC_CHAIN.into())
            .await
            .unwrap();
        let refund_address = bitcoin::Address::from_str(&refund_address_str)
            .unwrap()
            .assume_checked();
        let claim_address = elements::Address::from_str(&claim_address_str).unwrap();

        let response = session
            .btc_to_lbtc(50_000, &refund_address, &claim_address, None)
            .await
            .unwrap();

        let swap_id = response.swap_id().to_string();
        let lockup_address = response.lockup_address().to_string();
        let expected_amount = response.expected_amount();

        // Verify swap is in pending list
        let pending = session.pending_swap_ids().unwrap().unwrap();
        assert!(
            pending.contains(&swap_id),
            "Swap should be in pending list after creation"
        );

        // Verify swap data is stored
        let swap_data = session.get_swap_data(&swap_id).unwrap();
        assert!(swap_data.is_some(), "Swap data should be stored");

        // Drop the session and response
        drop(response);
        drop(session);

        // Create a new session with the same store
        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .mnemonic(mnemonic)
            .store(store.clone())
            .build()
            .await
            .unwrap();

        // Verify swap is still in pending list
        let pending = session.pending_swap_ids().unwrap().unwrap();
        assert!(
            pending.contains(&swap_id),
            "Swap should still be in pending list after session restart"
        );

        // Restore the swap from store data
        let swap_data_json = session.get_swap_data(&swap_id).unwrap().unwrap();
        let data = lwk_boltz::ChainSwapDataSerializable::deserialize(&swap_data_json).unwrap();
        let response = session.restore_lockup(data).await.unwrap();

        log::info!(
            "Restored BTC to LBTC swap with store - Lockup address: {}",
            response.lockup_address()
        );

        // Send funds and complete the swap
        crate::utils::send_to_address(BTC_CHAIN.into(), &lockup_address, expected_amount)
            .await
            .unwrap();

        let success = response.complete().await.unwrap();
        assert!(
            success,
            "Restored BTC to LBTC swap with store should succeed"
        );

        // Verify swap moved from pending to completed
        let pending = session.pending_swap_ids().unwrap().unwrap();
        let completed = session.completed_swap_ids().unwrap().unwrap();
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
        let pending = session.pending_swap_ids().unwrap().unwrap();
        let completed = session.completed_swap_ids().unwrap().unwrap();
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
    async fn test_session_onchain() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let _mining_handle = crate::utils::start_block_mining();

        let network = ElementsNetwork::default_regtest();
        let client =
            Arc::new(ElectrumClient::new(DEFAULT_REGTEST_NODE, false, false, network).unwrap());

        let session = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .create_swap_timeout(TIMEOUT)
            .build()
            .await
            .unwrap();

        // Test BTC to LBTC swap with quote verification
        let send_amount = 50_000u64;

        // Get quote before creating the swap
        let quote = session
            .quote(send_amount)
            .await
            .send(SwapAsset::Onchain)
            .receive(SwapAsset::Liquid)
            .build()
            .unwrap();
        log::info!(
            "BTC->LBTC Quote: send={}, receive={}, network_fee={}, boltz_fee={}",
            quote.send_amount,
            quote.receive_amount,
            quote.network_fee,
            quote.boltz_fee
        );

        let refund_address_str = crate::utils::generate_address(BTC_CHAIN.into())
            .await
            .unwrap();
        let claim_address_str = crate::utils::generate_address(LBTC_CHAIN.into())
            .await
            .unwrap();
        let refund_address = bitcoin::Address::from_str(&refund_address_str)
            .unwrap()
            .assume_checked();
        let claim_address = elements::Address::from_str(&claim_address_str).unwrap();
        let response = session
            .btc_to_lbtc(send_amount, &refund_address, &claim_address, None)
            .await
            .unwrap();

        // Verify quote matches swap response
        // For chain swaps: user sends lockup_amount, receives claim_details.amount - claim_fee
        let claim_fee = response.data.claim_fee.expect("claim_fee should be set");
        let claim_amount = response.data.create_chain_response.claim_details.amount;
        // For BTC→L-BTC, claim is on Liquid so we add LIQUID_UNCOOPERATIVE_EXTRA
        let expected_receive = claim_amount - claim_fee - LIQUID_UNCOOPERATIVE_EXTRA;
        assert_eq!(
            expected_receive, quote.receive_amount,
            "Quote receive_amount ({}) should match claim_amount ({}) - claim_fee ({}) - extra (3) = {}",
            quote.receive_amount, claim_amount, claim_fee, expected_receive
        );
        log::info!(
            "BTC->LBTC Quote verification passed: claim_amount={}, claim_fee={}, expected_receive={}",
            claim_amount,
            claim_fee,
            expected_receive
        );

        // Assert fees are present and reasonable
        let fee = response.fee().expect("fee should be present");
        let boltz_fee = response.boltz_fee().expect("boltz_fee should be present");
        log::info!("BTC to LBTC swap - fee: {fee}, boltz_fee: {boltz_fee}");
        assert!(fee > 0, "fee should be greater than 0");
        assert!(boltz_fee > 0, "boltz_fee should be greater than 0");
        assert!(fee > boltz_fee, "fee should be greater than boltz_fee");
        // Boltz fee is typically a percentage of the amount (e.g., 0.1-0.5%)
        // For 50,000 sats, expect boltz_fee to be roughly 50-250 sats
        assert!(
            boltz_fee < 500,
            "boltz_fee should be less than 1% of amount"
        );

        log::info!(
            "BTC to LBTC swap - Lockup address: {}",
            response.lockup_address()
        );
        crate::utils::send_to_address(
            BTC_CHAIN.into(),
            response.lockup_address(),
            response.expected_amount(),
        )
        .await
        .unwrap();
        let success = response.complete().await.unwrap();
        assert!(success, "BTC to LBTC swap should succeed");

        // Verify actual received balance matches quote
        let actual_balance =
            crate::utils::get_address_balance(LBTC_CHAIN.into(), &claim_address_str)
                .await
                .expect("Failed to get address balance");
        assert_eq!(
            actual_balance, quote.receive_amount,
            "BTC->LBTC: Actual received balance ({}) should match quote.receive_amount ({})",
            actual_balance, quote.receive_amount
        );
        log::info!(
            "BTC->LBTC Balance verification passed: actual_balance={}, quote.receive_amount={}",
            actual_balance,
            quote.receive_amount
        );

        // Test LBTC to BTC swap with quote verification
        let quote = session
            .quote(send_amount)
            .await
            .send(SwapAsset::Liquid)
            .receive(SwapAsset::Onchain)
            .build()
            .unwrap();
        log::info!(
            "LBTC->BTC Quote: send={}, receive={}, network_fee={}, boltz_fee={}",
            quote.send_amount,
            quote.receive_amount,
            quote.network_fee,
            quote.boltz_fee
        );

        let refund_address_str = crate::utils::generate_address(LBTC_CHAIN.into())
            .await
            .unwrap();
        let claim_address_str = crate::utils::generate_address(BTC_CHAIN.into())
            .await
            .unwrap();
        let refund_address = elements::Address::from_str(&refund_address_str).unwrap();
        let claim_address = bitcoin::Address::from_str(&claim_address_str)
            .unwrap()
            .assume_checked();
        let response = session
            .lbtc_to_btc(send_amount, &refund_address, &claim_address, None)
            .await
            .unwrap();

        // Verify quote matches swap response
        // For LBTC→BTC, claim is on Bitcoin so NO LIQUID_UNCOOPERATIVE_EXTRA
        let claim_fee = response.data.claim_fee.expect("claim_fee should be set");
        let claim_amount = response.data.create_chain_response.claim_details.amount;
        let expected_receive = claim_amount - claim_fee;
        assert_eq!(
            expected_receive, quote.receive_amount,
            "Quote receive_amount ({}) should match claim_amount ({}) - claim_fee ({}) = {}",
            quote.receive_amount, claim_amount, claim_fee, expected_receive
        );
        log::info!(
            "LBTC->BTC Quote verification passed: claim_amount={}, claim_fee={}, expected_receive={}",
            claim_amount,
            claim_fee,
            expected_receive
        );

        // Assert fees are present and reasonable
        let fee = response.fee().expect("fee should be present");
        let boltz_fee = response.boltz_fee().expect("boltz_fee should be present");
        log::info!("LBTC to BTC swap - fee: {fee}, boltz_fee: {boltz_fee}");
        assert!(fee > 0, "fee should be greater than 0");
        assert!(boltz_fee > 0, "boltz_fee should be greater than 0");
        assert!(fee > boltz_fee, "fee should be greater than boltz_fee");
        // Boltz fee is typically a percentage of the amount (e.g., 0.1-0.5%)
        // For 50,000 sats, expect boltz_fee to be roughly 50-250 sats
        assert!(
            boltz_fee < 500,
            "boltz_fee should be less than 1% of amount"
        );

        log::info!(
            "LBTC to BTC swap - Lockup address: {}",
            response.lockup_address()
        );
        crate::utils::send_to_address(
            LBTC_CHAIN.into(),
            response.lockup_address(),
            response.expected_amount(),
        )
        .await
        .unwrap();
        let success = response.complete().await.unwrap();
        assert!(success, "LBTC to BTC swap should succeed");

        // Verify actual received balance matches quote
        let actual_balance =
            crate::utils::get_address_balance(BTC_CHAIN.into(), &claim_address_str)
                .await
                .expect("Failed to get address balance");
        assert_eq!(
            actual_balance, quote.receive_amount,
            "LBTC->BTC: Actual received balance ({}) should match quote.receive_amount ({})",
            actual_balance, quote.receive_amount
        );
        log::info!(
            "LBTC->BTC Balance verification passed: actual_balance={}, quote.receive_amount={}",
            actual_balance,
            quote.receive_amount
        );

        // Test LBTC to BTC swap using advance()
        let refund_address_str = crate::utils::generate_address(LBTC_CHAIN.into())
            .await
            .unwrap();
        let claim_address_str = crate::utils::generate_address(BTC_CHAIN.into())
            .await
            .unwrap();
        let refund_address = elements::Address::from_str(&refund_address_str).unwrap();
        let claim_address = bitcoin::Address::from_str(&claim_address_str)
            .unwrap()
            .assume_checked();
        let mut response = session
            .lbtc_to_btc(50_000, &refund_address, &claim_address, None)
            .await
            .unwrap();
        log::info!(
            "LBTC to BTC swap - Lockup address: {}",
            response.lockup_address()
        );
        crate::utils::send_to_address(
            LBTC_CHAIN.into(),
            response.lockup_address(),
            response.expected_amount(),
        )
        .await
        .unwrap();
        loop {
            match response.advance().await {
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
        // repeatly calling advance on a terminated swap don't timeout
        for _ in 0..10 {
            match response.advance().await {
                Err(lwk_boltz::Error::NoBoltzUpdate) => { // expected
                }
                _ => {
                    panic!("unexpected status");
                }
            }
        }

        // Test polling mode
        let session_polling = BoltzSession::builder(network, AnyClient::Electrum(client.clone()))
            .polling(true)
            .bitcoin_electrum_client(&format!("tcp://{DEFAULT_REGTEST_NODE}")) // it's the same endpoint, just testing the builder setting
            .unwrap()
            .build()
            .await
            .unwrap();

        let refund_address_str = crate::utils::generate_address(BTC_CHAIN.into())
            .await
            .unwrap();
        let claim_address_str = crate::utils::generate_address(LBTC_CHAIN.into())
            .await
            .unwrap();
        let refund_address = bitcoin::Address::from_str(&refund_address_str)
            .unwrap()
            .assume_checked();
        let claim_address = elements::Address::from_str(&claim_address_str).unwrap();
        let mut response = session_polling
            .btc_to_lbtc(50_000, &refund_address, &claim_address, None)
            .await
            .unwrap();

        log::info!(
            "Polling BTC to LBTC swap - Lockup address: {}",
            response.lockup_address()
        );
        crate::utils::send_to_address(
            BTC_CHAIN.into(),
            response.lockup_address(),
            response.expected_amount(),
        )
        .await
        .unwrap();

        // Poll for updates until swap is complete
        loop {
            match response.advance().await {
                Ok(std::ops::ControlFlow::Continue(update)) => {
                    log::info!("Polling: Received update. status:{}", update.status);
                }
                Ok(std::ops::ControlFlow::Break(result)) => {
                    log::info!("Polling: Swap completed with result: {result}");
                    assert!(result, "Polling swap should succeed");
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
    }
}
