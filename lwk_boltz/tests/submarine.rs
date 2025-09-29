mod utils;

#[cfg(test)]
mod tests {

    use crate::utils::{self, BOLTZ_REGTEST, DEFAULT_REGTEST_NODE, TIMEOUT, WAIT_TIME};
    use std::sync::Arc;

    use boltz_client::{
        boltz::{BoltzApiClientV2, BoltzWsConfig, CreateSubmarineRequest},
        fees::Fee,
        network::{Chain, LiquidChain},
        swaps::{ChainClient, SwapScript, SwapTransactionParams},
        util::sleep,
        Keypair, PublicKey, Secp256k1,
    };
    use lwk_boltz::{clients::ElectrumClient, EventHandlerImpl, LighthingSession};
    use lwk_wollet::{secp256k1::rand::thread_rng, ElementsNetwork};

    #[tokio::test]
    async fn test_session_submarine() {
        let _ = env_logger::try_init();
        let session = LighthingSession::new(
            ElementsNetwork::default_regtest(),
            ElectrumClient::new(
                DEFAULT_REGTEST_NODE,
                false,
                false,
                ElementsNetwork::default_regtest(),
            )
            .unwrap(),
            Box::new(EventHandlerImpl {}),
        );
        let bolt11_invoice = utils::generate_invoice_lnd(50_000).await.unwrap();
        session.prepare_pay(&bolt11_invoice).await.unwrap();
    }

    #[tokio::test]
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
        let underpay = false;
        v2_submarine(&chain_client, underpay, chain).await;
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
