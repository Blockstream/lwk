pub mod blocking;
pub mod clients;
mod error;
mod invoice_data;
mod prepare_pay_data;
mod reverse;
mod submarine;
mod swap_state;

use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use bip39::Mnemonic;
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
use boltz_client::Keypair;
use boltz_client::Secp256k1;
use lwk_wollet::bitcoin::bip32::ChildNumber;
use lwk_wollet::bitcoin::bip32::DerivationPath;
use lwk_wollet::bitcoin::bip32::Xpriv;
use lwk_wollet::bitcoin::bip32::Xpub;
use lwk_wollet::bitcoin::NetworkKind;
use lwk_wollet::secp256k1::All;
use lwk_wollet::ElementsNetwork;
use serde::{Deserialize, Serialize};

use crate::clients::ElectrumClient;
pub use crate::error::Error;
pub use crate::invoice_data::InvoiceData;
pub use crate::prepare_pay_data::PreparePayData;
pub use crate::reverse::InvoiceResponse;
pub use crate::submarine::PreparePayResponse;
pub use crate::swap_state::SwapState;
pub use boltz_client::Bolt11Invoice;

pub(crate) const WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum SwapType {
    /// Pay a bolt11 invoice
    Submarine,

    /// Show an invoice to be paid
    Reverse,
}

pub struct LightningSession {
    ws: Arc<BoltzWsApi>,
    api: Arc<BoltzApiClientV2>,
    chain_client: Arc<ChainClient>,
    liquid_chain: LiquidChain,
    timeout: Duration,
    secp: Secp256k1<All>,

    #[allow(dead_code)]
    mnemonic: Mnemonic,
    #[allow(dead_code)]
    next_index_to_use: AtomicU32,
}

impl LightningSession {
    /// Create a new LighthingSession that connects to the Boltz API and starts a WebSocket connection
    ///
    /// Accept a `timeout` parameter to set the timeout for the Boltz API and WebSocket connection.
    /// If `timeout` is `None`, the default timeout of 10 seconds is used.
    ///
    pub async fn new(
        network: ElementsNetwork,
        client: Arc<ElectrumClient>, // TODO: should be generic to support other clients
        timeout: Option<Duration>,
        mnemonic: Option<Mnemonic>,
    ) -> Self {
        let liquid_chain = elements_network_to_liquid_chain(network);
        let chain_client = Arc::new(ChainClient::new().with_liquid((*client).clone()));
        let url = boltz_default_url(network);
        let api = Arc::new(BoltzApiClientV2::new(url.to_string(), timeout));
        let config = BoltzWsConfig::default();
        let ws_url = url.replace("http", "ws") + "/ws"; // api.get_ws_url() is private
        let ws = Arc::new(BoltzWsApi::new(ws_url, config));
        let future = BoltzWsApi::run_ws_loop(ws.clone());
        tokio::spawn(future); // TODO handle wasm
        let secp = Secp256k1::new();

        let (next_index_to_use, mnemonic) = match mnemonic {
            Some(mnemonic) => (
                fetch_next_index_to_use(&mnemonic, &secp, network_kind(liquid_chain), &api).await,
                mnemonic,
            ),
            None => (0, Mnemonic::generate(12).expect("12 is a valid word count")),
        };
        Self {
            next_index_to_use: AtomicU32::new(next_index_to_use),
            mnemonic,
            ws,
            api,
            chain_client,
            liquid_chain,
            timeout: timeout.unwrap_or(Duration::from_secs(10)),
            secp,
        }
    }

    fn chain(&self) -> Chain {
        Chain::Liquid(self.liquid_chain)
    }

    fn network(&self) -> ElementsNetwork {
        liquid_chain_to_elements_network(self.liquid_chain)
    }

    fn derive_next_keypair(&self) -> Result<Keypair, Error> {
        let index = self.next_index_to_use.fetch_add(1, Ordering::Relaxed);
        derive_keypair(index, &self.mnemonic, &self.secp)
    }

    /// Generate a rescue file with the lightning session mnemonic.
    ///
    /// The rescue file is a JSON file that contains the swaps mnemonic.
    /// It can be used on the Boltz web app to bring non terminated swaps to completition.
    pub fn rescue_file(&self) -> RescueFile {
        RescueFile {
            mnemonic: self.mnemonic.to_string(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct RescueFile {
    mnemonic: String,
}

fn network_kind(liquid_chain: LiquidChain) -> NetworkKind {
    if liquid_chain == LiquidChain::Liquid {
        NetworkKind::Main
    } else {
        NetworkKind::Test
    }
}

async fn fetch_next_index_to_use(
    mnemonic: &Mnemonic,
    secp: &Secp256k1<All>,
    network_kind: NetworkKind,
    client: &BoltzApiClientV2,
) -> u32 {
    let xpub = derive_xpub_from_mnemonic(mnemonic, secp, network_kind);
    log::info!("xpub for restore is: {}", xpub);

    let result = client.post_swap_restore(&xpub.to_string()).await.unwrap();
    log::info!("swap_restore api returns {} elements", result.len());

    let next_index_to_use = match result
        .iter()
        .filter_map(|e| {
            e.claim_details
                .as_ref()
                .map(|d| d.key_index)
                .or_else(|| e.refund_details.as_ref().map(|d| d.key_index))
        })
        .max()
    {
        Some(index) => index + 1,
        None => 0,
    };

    log::info!("next index to use is: {next_index_to_use}");
    next_index_to_use
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

/// Derive the master xpub from a mnemonic
fn derive_xpub_from_mnemonic(
    mnemonic: &Mnemonic,
    secp: &Secp256k1<All>,
    network_kind: NetworkKind,
) -> Xpub {
    let seed = mnemonic.to_seed("");
    let xpriv = Xpriv::new_master(network_kind, &seed[..]).unwrap();
    let derivation_path = DerivationPath::master();
    let derived = xpriv.derive_priv(&secp, &derivation_path).unwrap();
    Xpub::from_priv(&secp, &derived)
}

pub fn boltz_default_url(network: ElementsNetwork) -> &'static str {
    match network {
        ElementsNetwork::Liquid => BOLTZ_MAINNET_URL_V2,
        ElementsNetwork::LiquidTestnet => BOLTZ_TESTNET_URL_V2,
        ElementsNetwork::ElementsRegtest { .. } => BOLTZ_REGTEST,
    }
}

/// Wait for one of the expected swap status updates from a broadcast receiver with timeout
///
/// Note if there are concurrent swaps the broadcast receiver will receive updates for ALL swaps and
/// thus we filter out updates for other swaps.
pub async fn next_status(
    rx: &mut tokio::sync::broadcast::Receiver<SwapStatus>,
    timeout: Duration,
    expected_states: &[SwapState],
    swap_id: &str,
    last_state: SwapState,
) -> Result<SwapStatus, Error> {
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        // since we can receive updates for all swaps, we need to check the deadline
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let update = tokio::select! {
            update = rx.recv() => update?,
            _ = tokio::time::sleep(remaining) => {
                log::warn!("Timeout while waiting state {:?} for swap id {}", expected_states, swap_id );
                return Err(Error::Timeout(swap_id.to_string()));
            }
        };

        // Filter out updates for other swaps
        if update.id != swap_id {
            log::debug!(
                "Ignoring update for different swap: {} (waiting for {})",
                update.id,
                swap_id
            );
            continue;
        }

        log::info!(
            "Received update on swap {swap_id}. status:{}",
            update.status
        );
        let status = update
            .status
            .parse::<SwapState>()
            .map_err(|_| Error::UnexpectedUpdate {
                swap_id: swap_id.to_string(),
                status: update.status.clone(),
                last_state,
                expected_states: expected_states.to_vec(),
            })?;
        if !expected_states.contains(&status) {
            return Err(Error::UnexpectedUpdate {
                swap_id: swap_id.to_string(),
                status: update.status.clone(),
                last_state,
                expected_states: expected_states.to_vec(),
            });
        }

        return Ok(update);
    }
}

/// Derive a keypair from a mnemonic and index using the Boltz derivation path
///
/// This derivation path is a constant for Boltz, by using this we are compatible with the web app and can use the same rescue file
fn derive_keypair(
    index: u32,
    mnemonic: &Mnemonic,
    secp: &Secp256k1<All>,
) -> Result<Keypair, Error> {
    // Boltz derivation path: m/44/0/0/0/{index}
    let derivation_path = DerivationPath::from(vec![
        ChildNumber::from_normal_idx(44)?,
        ChildNumber::from_normal_idx(0)?,
        ChildNumber::from_normal_idx(0)?,
        ChildNumber::from_normal_idx(0)?,
        ChildNumber::from_normal_idx(index)?,
    ]);

    let seed = mnemonic.to_seed("");
    let xpriv = Xpriv::new_master(NetworkKind::Test, &seed[..])?; // the network is ininfluent since we don't use the extended key version
    let derived = xpriv.derive_priv(&secp, &derivation_path)?;
    log::info!("derive_next_keypair with index: {index}");
    let keypair = Keypair::from_seckey_slice(&secp, &derived.private_key.secret_bytes())?;
    Ok(keypair)
}

#[cfg(test)]
mod tests {
    use bip39::Mnemonic;
    use boltz_client::Secp256k1;
    use lwk_wollet::bitcoin::NetworkKind;

    use crate::derive_xpub_from_mnemonic;

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

    #[test]
    fn test_derive_xpub_from_mnemonic() {
        // from the web app
        let mnemonic = "damp cart merit asset obvious idea chef traffic absent armed road link";
        let expected_xpub = "xpub661MyMwAqRbcGprhd8RLPkaDpHxrJxiSWUUibirDPMnsvmUTW3djk2S3wsaz21ASEdw4uXQAypXA4CZ9u5EhCnXtLgfwck5PwXNRgvcaDUm";

        let mnemonic: Mnemonic = mnemonic.parse().unwrap();
        let secp = Secp256k1::new();
        let network_kind = NetworkKind::Main;
        let xpub = derive_xpub_from_mnemonic(&mnemonic, &secp, network_kind);
        assert_eq!(xpub.to_string(), expected_xpub);
    }

    #[test]
    fn test_derive_keypair() {
        // from the web app
        let mnemonic = "damp cart merit asset obvious idea chef traffic absent armed road link";
        let expected_keypair_pubkey =
            "0315a98cf1610e96ca92505c6e9536a208353399685440869dca58947a909d07ed";

        let mnemonic: Mnemonic = mnemonic.parse().unwrap();
        let secp = Secp256k1::new();
        let index = 0;
        let keypair = crate::derive_keypair(index, &mnemonic, &secp).unwrap();
        assert_eq!(keypair.public_key().to_string(), expected_keypair_pubkey);
    }
}
