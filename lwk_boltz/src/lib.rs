pub mod clients;
mod error;
mod submarine;

use std::sync::Arc;

use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::BoltzWsApi;
use boltz_client::boltz::BoltzWsConfig;
use boltz_client::boltz::CreateReverseRequest;
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
pub use crate::error::Error;

pub struct LightningSession {
    ws: Arc<BoltzWsApi>,
    api: Arc<BoltzApiClientV2>,
    chain_client: Arc<ChainClient>,
    liquid_chain: LiquidChain,
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

        let reverse_resp = self.api.post_reverse_req(create_reverse_req).await?;
        let invoice = reverse_resp
            .invoice
            .as_ref()
            .ok_or(Error::MissingInvoiceInResponse(reverse_resp.id.clone()))?
            .clone();

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
