mod utils;

#[cfg(test)]
mod tests {
    use crate::utils::{self, TIMEOUT, WAIT_TIME};
    use crate::utils::{next_status, DEFAULT_REGTEST_NODE};
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
    use lwk_boltz::clients::ElectrumClient;
    use lwk_wollet::secp256k1::rand::thread_rng;
    use lwk_wollet::ElementsNetwork;
    use std::sync::Arc;

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
    async fn test_chain_swaps_btc_lbtc() {
        let chain_client = create_chain_client_electrum();
        v2_chain(&chain_client, false, BTC_CHAIN.into(), LBTC_CHAIN.into()).await;
        v2_chain(&chain_client, true, BTC_CHAIN.into(), LBTC_CHAIN.into()).await;
    }
    #[tokio::test]
    async fn test_chain_swaps_lbtc_btc() {
        let chain_client = create_chain_client_electrum();
        v2_chain(&chain_client, false, LBTC_CHAIN.into(), BTC_CHAIN.into()).await;
        v2_chain(&chain_client, true, LBTC_CHAIN.into(), BTC_CHAIN.into()).await;
    }

    async fn v2_chain(chain_client: &ChainClient, underpay: bool, from: Chain, to: Chain) {
        let _ = env_logger::try_init();

        let secp = Secp256k1::new();
        let preimage = Preimage::new();
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
}
