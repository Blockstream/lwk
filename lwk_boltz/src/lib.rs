pub mod clients;

use std::sync::Arc;

use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::BoltzWsApi;
use boltz_client::boltz::BoltzWsConfig;
use boltz_client::boltz::CreateReverseRequest;
use boltz_client::boltz::CreateSubmarineRequest;
use boltz_client::boltz::BOLTZ_MAINNET_URL_V2;
use boltz_client::boltz::BOLTZ_REGTEST;
use boltz_client::boltz::BOLTZ_TESTNET_URL_V2;
use boltz_client::fees::Fee;
use boltz_client::network::Chain;
use boltz_client::network::LiquidChain;
use boltz_client::swaps::magic_routing::check_for_mrh;
use boltz_client::swaps::magic_routing::sign_address;
use boltz_client::swaps::ChainClient;
use boltz_client::swaps::SwapScript;
use boltz_client::swaps::SwapTransactionParams;
use boltz_client::swaps::TransactionOptions;
use boltz_client::util::secrets::Preimage;
use boltz_client::util::sleep;
use boltz_client::Secp256k1;
use boltz_client::{Keypair, PublicKey};
use lwk_wollet::secp256k1::rand::thread_rng;
use lwk_wollet::ElementsNetwork;

use crate::clients::ElectrumClient;

pub struct LightningSession {
    ws: Arc<BoltzWsApi>,
    api: Arc<BoltzApiClientV2>,
    chain_client: Arc<ChainClient>,
    liquid_chain: LiquidChain,
}

#[derive(Debug)]
pub enum Error {
    InvalidInvoice,
}

impl LightningSession {
    /// Create a new LighthingSession that connects to the Boltz API and starts a WebSocket connection
    // TODO: add mnemonic as param to generate deterministic keypairs
    pub fn new(
        network: ElementsNetwork,
        client: ElectrumClient, // TODO: should be generic to support other clients
    ) -> Self {
        let chain_client = Arc::new(ChainClient::new().with_liquid(client));
        let url = boltz_default_url(network);
        let api = Arc::new(BoltzApiClientV2::new(url.to_string(), None)); // TODO: implement timeout
        let config = BoltzWsConfig::default();
        let ws_url = url.replace("http", "ws") + "/ws"; // api.get_ws_url() is private
        let ws = Arc::new(BoltzWsApi::new(ws_url, config));
        let future = BoltzWsApi::run_ws_loop(ws.clone());
        tokio::spawn(future); // TODO handle wasm
        Self {
            ws,
            api,
            chain_client,
            liquid_chain: elements_network_to_liquid_chain(network),
        }
    }

    fn chain(&self) -> Chain {
        Chain::Liquid(self.liquid_chain)
    }

    pub async fn prepare_pay(
        &self,
        bolt11_invoice: &str,
        // refund_address: elements::Address,
    ) -> Result<PreparePayResponse, Error> {
        let chain = self.chain();

        let secp = Secp256k1::new();
        let our_keys = Keypair::new(&secp, &mut thread_rng());
        let refund_public_key = PublicKey {
            inner: our_keys.public_key(),
            compressed: true,
        };

        let create_swap_req = CreateSubmarineRequest {
            from: chain.to_string(),
            to: "BTC".to_string(),
            invoice: bolt11_invoice.to_string(),
            refund_public_key,
            pair_hash: None,
            referral_id: None,
            webhook: None,
        };

        let create_swap_response = self.api.post_swap_req(&create_swap_req).await.unwrap();

        log::info!("Got Swap Response from Boltz server {create_swap_response:?}");

        create_swap_response
            .validate(&bolt11_invoice, &refund_public_key, chain)
            .unwrap();
        log::info!("VALIDATED RESPONSE!");

        let swap_script =
            SwapScript::submarine_from_swap_resp(chain, &create_swap_response, refund_public_key)
                .unwrap();
        let swap_id = create_swap_response.id.clone();
        log::info!("Created Swap Script id:{swap_id} swap_script:{swap_script:?}");

        let mut rx = self.ws.updates();
        self.ws.subscribe_swap(&swap_id).await.unwrap();
        let update = rx.recv().await.unwrap();
        match update.status.as_str() {
            "invoice.set" => {
                log::info!(
                    "Send {} sats to {} address {}",
                    create_swap_response.expected_amount,
                    chain,
                    create_swap_response.address
                );
                Ok(PreparePayResponse {
                    swap_id,
                    uri: create_swap_response.bip21,
                    address: create_swap_response.address,
                    amount: create_swap_response.expected_amount,
                    fee: 0, // TODO: populate fee correctly
                    rx,
                    swap_script,
                    api: self.api.clone(),
                    our_keys,
                    bolt11_invoice: bolt11_invoice.to_string(),
                })
            }
            _ => {
                panic!("Unexpected update: {}", update.status);
            }
        }
    }

    pub async fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: String, // TODO: use elements::Address
    ) -> Result<InvoiceResponse, Error> {
        let chain = self.chain();
        let secp = Secp256k1::new();
        let preimage = Preimage::new();
        let our_keys = Keypair::new(&secp, &mut thread_rng());
        let claim_public_key = PublicKey {
            compressed: true,
            inner: our_keys.public_key(),
        };

        let addrs_sig = sign_address(&claim_address, &our_keys).unwrap();
        let create_reverse_req = CreateReverseRequest {
            from: "BTC".to_string(),
            to: chain.to_string(),
            invoice: None,
            invoice_amount: Some(amount),
            preimage_hash: Some(preimage.sha256),
            description,
            description_hash: None,
            address_signature: Some(addrs_sig.to_string()),
            address: Some(claim_address.clone()),
            claim_public_key,
            referral_id: None, // Add address signature here.
            webhook: None,
        };

        let reverse_resp = self.api.post_reverse_req(create_reverse_req).await.unwrap();
        let invoice = reverse_resp.invoice.clone().unwrap();

        let _ = check_for_mrh(&self.api, &invoice, chain)
            .await
            .unwrap()
            .unwrap();

        log::debug!("Got Reverse swap response: {reverse_resp:?}");

        let swap_script =
            SwapScript::reverse_from_swap_resp(chain, &reverse_resp, claim_public_key).unwrap();
        let swap_id = reverse_resp.id.clone();

        self.ws.subscribe_swap(&swap_id).await.unwrap();
        let mut rx = self.ws.updates();

        // TODO "swap.created"
        let update = rx.recv().await.unwrap();
        match update.status.as_str() {
            "swap.created" => {
                log::info!("Waiting for Invoice to be paid: {}", &invoice);
            }
            _ => {
                panic!("Unexpected update: {}", update.status);
            }
        }

        Ok(InvoiceResponse {
            swap_id,
            bolt11_invoice: invoice,
            swap_fee: 0,    // TODO: populate fee correctly
            network_fee: 0, // TODO: populate fee correctly
            rx,
            swap_script,
            api: self.api.clone(),
            our_keys,
            preimage,
            claim_address,
            chain_client: self.chain_client.clone(),
        })
    }
}

#[derive(Debug)]
pub struct PreparePayResponse {
    pub swap_id: String,

    /// A liquidnetwork uri with the address to pay and the amount.
    /// Note the amount is greater that what is specified in the bolt11 invoice because of fees
    pub uri: String,

    /// The address to pay to.
    /// It is the same contained in the uri but provided for convenience.
    pub address: String,

    /// The amount to pay.
    /// It is the same contained in the uri but provided for convenience.
    pub amount: u64,

    /// Fee in satoshi, it's equal to the `amount` less the bolt11 amount
    pub fee: u64,

    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
    bolt11_invoice: String,
    swap_script: SwapScript,
    api: Arc<BoltzApiClientV2>,
    our_keys: Keypair,
}

pub struct InvoiceResponse {
    pub swap_id: String,
    /// The invoice to show to the payer, the invoice amount will be exactly like the amount parameter,
    /// However, the receiver will receive `amount - swap_fee - network_fee`
    pub bolt11_invoice: String,

    /// The fee of the swap provider
    pub swap_fee: u64,

    /// The network fee (fee of the onchain transaction)
    pub network_fee: u64,

    rx: tokio::sync::broadcast::Receiver<boltz_client::boltz::SwapStatus>,
    swap_script: SwapScript,
    api: Arc<BoltzApiClientV2>,
    our_keys: Keypair,
    preimage: Preimage,
    claim_address: String,
    chain_client: Arc<ChainClient>,
}

impl PreparePayResponse {
    pub async fn complete_pay(mut self) -> Result<bool, Error> {
        loop {
            let update = self.rx.recv().await.unwrap();
            log::info!("Got Update from server: {}", update.status);
            log::debug!("Update: {update:?}");
            match update.status.as_str() {
                "transaction.mempool" => {}
                "transaction.confirmed" => {}
                "invoice.pending" => {}
                "invoice.paid" => {}
                "transaction.claim.pending" => {
                    let response = self
                        .swap_script
                        .submarine_cooperative_claim(
                            &self.swap_id,
                            &self.our_keys,
                            &self.bolt11_invoice,
                            &self.api,
                        )
                        .await
                        .unwrap();
                    log::debug!("Received claim tx details : {response:?}");
                }

                "transaction.claimed" => {
                    break Ok(true);
                }

                // This means the funding transaction was rejected by Boltz for whatever reason, and we need to get
                // the funds back via refund.
                "transaction.lockupFailed" | "invoice.failedToPay" => {
                    todo!();
                    // sleep(WAIT_TIME).await;
                    // let tx = swap_script
                    //     .construct_refund(SwapTransactionParams {
                    //         keys: our_keys,
                    //         output_address: refund_address,
                    //         fee: Fee::Absolute(1000),
                    //         swap_id: swap_id.clone(),
                    //         chain_client,
                    //         boltz_client: &boltz_api_v2,
                    //         options: None,
                    //     })
                    //     .await
                    //     .unwrap();

                    // let txid = chain_client.broadcast_tx(&tx).await.unwrap();
                    // log::info!("Cooperative Refund Successfully broadcasted: {txid}");

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
                }
                _ => {
                    panic!("Unexpected update: {}", update.status);
                }
            };
        }
    }
}

impl InvoiceResponse {
    pub async fn complete_pay(mut self) -> Result<bool, Error> {
        loop {
            let update = self.rx.recv().await.unwrap();
            match update.status.as_str() {
                "transaction.mempool" => {
                    log::info!("Boltz broadcasted funding tx");

                    const WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(5);
                    sleep(WAIT_TIME).await; // TODO better way to wait

                    let tx = self
                        .swap_script
                        .construct_claim(
                            &self.preimage,
                            SwapTransactionParams {
                                keys: self.our_keys,
                                output_address: self.claim_address.clone(),
                                fee: Fee::Relative(1.0),
                                swap_id: self.swap_id.clone(),
                                options: Some(TransactionOptions::default().with_cooperative(true)),
                                chain_client: &self.chain_client,
                                boltz_client: &self.api,
                            },
                        )
                        .await
                        .unwrap();

                    self.chain_client.broadcast_tx(&tx).await.unwrap();

                    log::info!("Successfully broadcasted claim tx!");
                    log::debug!("Claim Tx {tx:?}");
                }
                "transaction.confirmed" => {}
                "invoice.settled" => {
                    log::info!("Reverse Swap Successful!");
                    break Ok(true);
                }
                _ => {
                    panic!("Unexpected update: {}", update.status);
                }
            }
            log::info!("Got Update from server: {}", update.status);
        }
    }
}

/// Convert an ElementsNetwork to a LiquidChain
pub fn elements_network_to_liquid_chain(network: ElementsNetwork) -> LiquidChain {
    match network {
        ElementsNetwork::Liquid => LiquidChain::Liquid,
        ElementsNetwork::LiquidTestnet => LiquidChain::LiquidTestnet,
        ElementsNetwork::ElementsRegtest { .. } => LiquidChain::LiquidRegtest,
    }
}

/// Convert a LiquidChain to an ElementsNetwork
pub fn liquid_chain_to_elements_network(chain: LiquidChain) -> ElementsNetwork {
    match chain {
        LiquidChain::Liquid => ElementsNetwork::Liquid,
        LiquidChain::LiquidTestnet => ElementsNetwork::LiquidTestnet,
        LiquidChain::LiquidRegtest => ElementsNetwork::default_regtest(),
    }
}

pub fn boltz_default_url(network: ElementsNetwork) -> &'static str {
    match network {
        ElementsNetwork::Liquid => BOLTZ_MAINNET_URL_V2,
        ElementsNetwork::LiquidTestnet => BOLTZ_TESTNET_URL_V2,
        ElementsNetwork::ElementsRegtest { .. } => BOLTZ_REGTEST,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_elements_network_to_liquid_chain() {
        // Test all networks with roundtrip conversion
        let networks = vec![
            lwk_wollet::ElementsNetwork::Liquid,
            lwk_wollet::ElementsNetwork::LiquidTestnet,
            lwk_wollet::ElementsNetwork::default_regtest(),
        ];

        for network in networks {
            // Test forward conversion
            let chain = crate::elements_network_to_liquid_chain(network.clone());
            // Test roundtrip: convert back and ensure it equals original
            let roundtrip_network = crate::liquid_chain_to_elements_network(chain);
            assert_eq!(network, roundtrip_network);
        }
    }
}
