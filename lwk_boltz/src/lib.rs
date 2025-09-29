pub mod clients;

use std::sync::Arc;

use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::BoltzWsApi;
use boltz_client::boltz::BoltzWsConfig;
use boltz_client::boltz::CreateSubmarineRequest;
use boltz_client::boltz::BOLTZ_MAINNET_URL_V2;
use boltz_client::boltz::BOLTZ_REGTEST;
use boltz_client::boltz::BOLTZ_TESTNET_URL_V2;
use boltz_client::network::Chain;
use boltz_client::network::LiquidChain;
use boltz_client::swaps::ChainClient;
use boltz_client::swaps::SwapScript;
use boltz_client::Secp256k1;
use boltz_client::{Keypair, PublicKey};
use lwk_wollet::secp256k1::rand::thread_rng;
use lwk_wollet::ElementsNetwork;

use crate::clients::ElectrumClient;

pub struct LighthingSession {
    ws: Arc<BoltzWsApi>,
    api: BoltzApiClientV2,
    chain_client: ChainClient,
    liquid_chain: LiquidChain,
}

#[derive(Debug)]
pub enum Error {
    InvalidInvoice,
}

impl LighthingSession {
    /// Create a new LighthingSession that connects to the Boltz API and starts a WebSocket connection
    // TODO: add mnemonic as param to generate deterministic keypairs
    pub fn new(
        network: ElementsNetwork,
        client: ElectrumClient, // TODO: should be generic to support other clients
        _handler: Box<dyn EventHandler>,
    ) -> Self {
        let chain_client = ChainClient::new().with_liquid(client);
        let url = boltz_default_url(network);
        let api = BoltzApiClientV2::new(url.to_string(), None); // TODO: implement timeout
        let config = BoltzWsConfig::default();
        let ws = Arc::new(BoltzWsApi::new(url.to_string(), config));
        let future = BoltzWsApi::run_ws_loop(ws.clone());
        tokio::spawn(future); // TODO handle wasm
        Self {
            ws,
            api,
            chain_client,
            liquid_chain: elements_network_to_liquid_chain(network),
        }
    }

    pub async fn prepare_pay(
        &self,
        bolt11_invoice: &str,
        // refund_address: elements::Address,
    ) -> Result<PreparePayResponse, Error> {
        let chain = Chain::Liquid(self.liquid_chain);

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

        // let mut rx = ws_api.updates();
        // ws_api.subscribe_swap(&swap_id).await.unwrap();
        todo!()
    }
}

pub struct PreparePayResponse {
    /// A liquidnetwork uri with the address to pay and the amount.
    /// Note the amount is greater that what is specified in the bolt11 invoice because of fees
    uri: String,

    /// Fee in satoshi, it's equal to the `amount` less the bolt11 amount
    fee: u64,
}

pub struct Event;
pub trait EventHandler {
    fn on_event(&self, e: Event);
}
pub struct EventHandlerImpl;
impl EventHandler for EventHandlerImpl {
    fn on_event(&self, e: Event) {
        todo!()
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
