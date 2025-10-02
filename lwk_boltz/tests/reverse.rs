mod utils;

#[cfg(test)]
mod tests {

    use crate::utils::{self, DEFAULT_REGTEST_NODE, TIMEOUT, WAIT_TIME};
    use std::{env, str::FromStr, sync::Arc};

    use boltz_client::{
        boltz::{BoltzApiClientV2, BoltzWsConfig, CreateReverseRequest, BOLTZ_REGTEST},
        fees::Fee,
        network::{Chain, LiquidChain},
        swaps::{
            magic_routing::{check_for_mrh, sign_address},
            ChainClient, SwapScript, SwapTransactionParams, TransactionOptions,
        },
        util::{secrets::Preimage, sleep},
        Keypair, PublicKey, Secp256k1,
    };
    use lwk_boltz::{clients::ElectrumClient, LightningSession};
    use lwk_wollet::{elements, secp256k1::rand::thread_rng, ElementsNetwork};

    #[tokio::test]
    #[ignore = "mainnet"]
    async fn test_session_create_invoice_mainnet() {
        let _ = env_logger::try_init();
        let network = ElementsNetwork::Liquid;
        let session = LightningSession::new(
            network,
            Arc::new(
                ElectrumClient::new(
                    "elements-mainnet.blockstream.info:50002",
                    true,
                    true,
                    network,
                )
                .unwrap(),
            ),
            Some(TIMEOUT),
        );
        let mainnet_addr = elements::Address::from_str("lq1qqvp9g33gw9y05xava3dvcpq8pnkv82yj3tdnzp547eyp9yrztz2lkyxrhscd55ev4p7lj2n72jtkn5u4xnj4v577c42jhf3ww").unwrap();
        log::info!("creating invoice for mainnet address: {}", mainnet_addr);

        for _ in 0..10 {
            let invoice_response = session
                .invoice(1000, Some("test".to_string()), &mainnet_addr)
                .await;
            match invoice_response {
                Ok(invoice_response) => {
                    assert!(invoice_response.bolt11_invoice.starts_with("lnbc1"));
                    return;
                }
                Err(e) => {
                    // it happens sometimes that the invoice is not created with:
                    // [2025-10-02T11:03:52Z WARN  boltz_client::swaps::status_stream] Failed to broadcast update: channel closed
                    // in this case we retry, testing the capability of the session to retry
                    log::error!("Error creating invoice: {:?}", e);
                }
            }
        }
        assert!(false, "Invoice not created after 10 attempts");
    }

    #[tokio::test]
    #[ignore = "mainnet"]
    async fn test_session_reverse_mainnet() {
        let _ = env_logger::try_init();

        use lwk_common::Signer;
        let mnemonic = env::var("MAINNET_MNEMONIC").unwrap();
        let network = ElementsNetwork::Liquid;
        let signer = lwk_signer::SwSigner::new(&mnemonic, true).unwrap();
        let desc = signer.wpkh_slip77_descriptor().unwrap();
        let desc: lwk_wollet::WolletDescriptor = desc.parse().unwrap();
        let claim_address = desc.address(2, network.address_params()).unwrap();
        log::info!("Claim Address: {}", claim_address);
        let client = ElectrumClient::new(
            "elements-mainnet.blockstream.info:50002",
            true,
            true,
            network,
        )
        .unwrap();
        let session = LightningSession::new(network, Arc::new(client), Some(TIMEOUT));
        let response = session
            .invoice(1000, Some("test".to_string()), &claim_address)
            .await
            .unwrap();
        log::info!("Invoice Response: {}", response.bolt11_invoice);
        log::info!("Waiting for invoice to be paid");
        let result = response.complete_pay().await;
        log::info!("Complete Pay Result: {:?}", result);
    }

    #[tokio::test]
    async fn test_reverse() {
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
        let cooperative = true;
        v2_reverse(&chain_client, chain, cooperative).await;
    }

    #[tokio::test]
    async fn test_session_reverse() {
        let _ = env_logger::try_init();

        // Start concurrent block mining task
        let _mining_handle = utils::start_block_mining();
        let network = ElementsNetwork::default_regtest();
        let client = ElectrumClient::new(DEFAULT_REGTEST_NODE, false, false, network).unwrap();

        let session = LightningSession::new(network, Arc::new(client), Some(TIMEOUT));
        let claim_address = utils::generate_address(Chain::Liquid(LiquidChain::LiquidRegtest))
            .await
            .unwrap();
        let claim_address = elements::Address::from_str(&claim_address).unwrap();
        let invoice = session.invoice(100000, None, &claim_address).await.unwrap();
        log::info!("Invoice: {}", invoice.bolt11_invoice);
        utils::start_pay_invoice_lnd(invoice.bolt11_invoice.clone());
        invoice.complete_pay().await.unwrap();
    }

    /// Test the reverse swap, copied from the boltz_client tests
    async fn v2_reverse(chain_client: &ChainClient, chain: Chain, cooperative: bool) {
        let secp = Secp256k1::new();
        let preimage = Preimage::new();
        let our_keys = Keypair::new(&secp, &mut thread_rng());
        let invoice_amount = 100000;
        let claim_public_key = PublicKey {
            compressed: true,
            inner: our_keys.public_key(),
        };

        log::info!("Generating claim address");
        // Give a valid claim address or else funds will be lost.
        let claim_address = utils::generate_address(chain).await.unwrap();

        let addrs_sig = sign_address(&claim_address, &our_keys).unwrap();
        let create_reverse_req = CreateReverseRequest {
            from: "BTC".to_string(),
            to: chain.to_string(),
            invoice: None,
            invoice_amount: Some(invoice_amount),
            preimage_hash: Some(preimage.sha256),
            description: None,
            description_hash: None,
            address_signature: Some(addrs_sig.to_string()),
            address: Some(claim_address.clone()),
            claim_public_key,
            referral_id: None, // Add address signature here.
            webhook: None,
        };

        let boltz_api_v2 = BoltzApiClientV2::new(BOLTZ_REGTEST.to_string(), Some(TIMEOUT));
        let ws_api = Arc::new(boltz_api_v2.ws(BoltzWsConfig::default()));
        utils::start_ws(ws_api.clone());

        let reverse_resp = boltz_api_v2
            .post_reverse_req(create_reverse_req)
            .await
            .unwrap();
        let invoice = reverse_resp.invoice.clone().unwrap();

        let _ = check_for_mrh(&boltz_api_v2, &invoice, chain)
            .await
            .unwrap()
            .unwrap();

        log::debug!("Got Reverse swap response: {reverse_resp:?}");

        let swap_script =
            SwapScript::reverse_from_swap_resp(chain, &reverse_resp, claim_public_key).unwrap();
        let swap_id = reverse_resp.id.clone();

        ws_api.subscribe_swap(&swap_id).await.unwrap();
        let mut rx = ws_api.updates();

        loop {
            let update = rx.recv().await.unwrap();
            match update.status.as_str() {
                "swap.created" => {
                    log::info!("Waiting for Invoice to be paid: {}", &invoice);

                    utils::start_pay_invoice_lnd(invoice.clone());

                    continue;
                }

                "transaction.mempool" => {
                    log::info!("Boltz broadcasted funding tx");

                    sleep(WAIT_TIME).await;

                    let tx = swap_script
                        .construct_claim(
                            &preimage,
                            SwapTransactionParams {
                                keys: our_keys,
                                output_address: claim_address.clone(),
                                fee: Fee::Absolute(1000),
                                swap_id: swap_id.clone(),
                                options: Some(
                                    TransactionOptions::default().with_cooperative(cooperative),
                                ),
                                chain_client,
                                boltz_client: &boltz_api_v2,
                            },
                        )
                        .await
                        .unwrap();

                    chain_client.broadcast_tx(&tx).await.unwrap();

                    log::info!("Successfully broadcasted claim tx!");
                    log::debug!("Claim Tx {tx:?}");
                }

                "invoice.settled" => {
                    log::info!("Reverse Swap Successful!");
                    break;
                }
                _ => {
                    log::info!("Got Update from server: {}", update.status);
                }
            }
        }
    }
}
