pub mod blocking;
pub mod clients;
mod error;
mod reverse;
mod submarine;
mod swap_state;

use std::sync::Arc;
use std::time::Duration;

use boltz_client::boltz::BoltzApiClientV2;
use boltz_client::boltz::BoltzWsApi;
use boltz_client::boltz::BoltzWsConfig;
use boltz_client::boltz::SwapStatus;
use boltz_client::boltz::BOLTZ_MAINNET_URL_V2;
use boltz_client::boltz::BOLTZ_REGTEST;
use boltz_client::boltz::BOLTZ_TESTNET_URL_V2;
use boltz_client::network::Chain;
use boltz_client::network::LiquidChain;
use boltz_client::swaps::ChainClient;
use lwk_wollet::ElementsNetwork;

use crate::clients::ElectrumClient;
pub use crate::error::Error;
pub use crate::reverse::InvoiceResponse;
pub use crate::submarine::{PreparePayData, PreparePayResponse};
pub use crate::swap_state::SwapState;
pub use boltz_client::Bolt11Invoice;

pub(crate) const WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(5);

pub struct LightningSession {
    ws: Arc<BoltzWsApi>,
    api: Arc<BoltzApiClientV2>,
    chain_client: Arc<ChainClient>,
    liquid_chain: LiquidChain,
    timeout: Duration,
}

impl LightningSession {
    /// Create a new LighthingSession that connects to the Boltz API and starts a WebSocket connection
    ///
    /// Accept a `timeout` parameter to set the timeout for the Boltz API and WebSocket connection.
    /// If `timeout` is `None`, the default timeout of 10 seconds is used.
    ///
    // TODO: add mnemonic as param to generate deterministic keypairs
    pub fn new(
        network: ElementsNetwork,
        client: Arc<ElectrumClient>, // TODO: should be generic to support other clients
        timeout: Option<Duration>,
    ) -> Self {
        let chain_client = Arc::new(ChainClient::new().with_liquid((*client).clone()));
        let url = boltz_default_url(network);
        let api = Arc::new(BoltzApiClientV2::new(url.to_string(), timeout));
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
            timeout: timeout.unwrap_or(Duration::from_secs(10)),
        }
    }

    fn chain(&self) -> Chain {
        Chain::Liquid(self.liquid_chain)
    }

    fn network(&self) -> ElementsNetwork {
        liquid_chain_to_elements_network(self.liquid_chain)
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

/// Wait for one of the expected swap status updates from a broadcast receiver with timeout
pub async fn next_status(
    rx: &mut tokio::sync::broadcast::Receiver<SwapStatus>,
    timeout: Duration,
    expected_states: &[SwapState],
    swap_id: &str,
) -> Result<SwapStatus, Error> {
    let update = tokio::select! {
        update = rx.recv() => update?,
        _ = tokio::time::sleep(timeout) => {
            log::warn!("Timeout while waiting state {:?} for swap id {}", expected_states, swap_id );
            return Err(Error::Timeout(swap_id.to_string()));
        }
    };
    log::info!("Received update. status:{}", update.status);
    let status = update
        .status
        .parse::<SwapState>()
        .map_err(|_| Error::UnexpectedUpdate {
            swap_id: swap_id.to_string(),
            status: update.status.clone(),
        })?;
    if !expected_states.contains(&status) {
        return Err(Error::UnexpectedUpdate {
            swap_id: swap_id.to_string(),
            status: update.status.clone(),
        });
    }

    Ok(update)
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
