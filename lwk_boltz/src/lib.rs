pub mod clients;

use std::sync::Arc;

use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::BoltzWsApi;
use boltz_client::boltz::BoltzWsConfig;
use boltz_client::boltz::BOLTZ_MAINNET_URL_V2;
use boltz_client::boltz::BOLTZ_REGTEST;
use boltz_client::boltz::BOLTZ_TESTNET_URL_V2;
use boltz_client::network::LiquidChain;
use boltz_client::network::LiquidClient;
use boltz_client::swaps::ChainClient;
use lwk_wollet::ElementsNetwork;

use crate::clients::ElectrumClient;

pub struct LighthingSession {
    ws: Arc<BoltzWsApi>,
    api: BoltzApiClientV2,
    chain_client: ChainClient,
}

enum Error {
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
        }
    }

    pub async fn prepare_pay(
        &self,
        bolt11_invoice: &str,
        refund_address: String, // TODO use elements::Address
    ) -> Result<PreparePayResponse, Error> {
        todo!()
    }
}

struct PreparePayResponse {
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
