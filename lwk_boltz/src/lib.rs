#![cfg_attr(not(test), deny(clippy::unwrap_used))]

pub mod blocking;
pub mod clients;
mod error;
mod invoice_data;
mod lightning_payment;
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
use boltz_client::boltz::GetReversePairsResponse;
use boltz_client::boltz::GetSubmarinePairsResponse;
use boltz_client::boltz::SwapStatus;
use boltz_client::boltz::BOLTZ_MAINNET_URL_V2;
use boltz_client::boltz::BOLTZ_REGTEST;
use boltz_client::boltz::BOLTZ_TESTNET_URL_V2;
use boltz_client::network::Chain;
use boltz_client::network::LiquidChain;
use boltz_client::swaps::ChainClient;
use boltz_client::util::sleep;
use boltz_client::Keypair;
use lightning::bitcoin::XKeyIdentifier;
use lwk_wollet::bitcoin::bip32::ChildNumber;
use lwk_wollet::bitcoin::bip32::DerivationPath;
use lwk_wollet::bitcoin::bip32::Xpriv;
use lwk_wollet::bitcoin::bip32::Xpub;
use lwk_wollet::bitcoin::NetworkKind;
use lwk_wollet::ElementsNetwork;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::TryRecvError;

use crate::clients::AnyClient;
pub use crate::error::Error;
pub use crate::invoice_data::to_invoice_data;
pub use crate::invoice_data::InvoiceData;
pub use crate::invoice_data::InvoiceDataSerializable;
pub use crate::lightning_payment::LightningPayment;
pub use crate::prepare_pay_data::PreparePayData;
pub use crate::prepare_pay_data::PreparePayDataSerializable;
pub use crate::reverse::InvoiceResponse;
pub use crate::submarine::PreparePayResponse;
pub use crate::swap_state::SwapState;
pub use boltz_client::boltz::{RevSwapStates, SubSwapStates, SwapRestoreResponse, Webhook};
pub use boltz_client::Bolt11Invoice;

pub use boltz_client::boltz::SwapRestoreType as SwapType;

pub(crate) const WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(5);

pub struct BoltzSession {
    ws: Arc<BoltzWsApi>,
    api: Arc<BoltzApiClientV2>,
    chain_client: Arc<ChainClient>,
    liquid_chain: LiquidChain,
    timeout: Duration,

    mnemonic: Mnemonic,
    next_index_to_use: AtomicU32,

    polling: bool,
    timeout_advance: Duration,
}

impl BoltzSession {
    /// Create a new BoltzSession with default settings
    ///
    /// This is a convenience method that uses default timeout (10 seconds)
    /// and generates a random mnemonic.
    ///
    /// For custom configuration, use [`BoltzSession::builder()`] instead.
    pub async fn new(network: ElementsNetwork, client: AnyClient) -> Result<Self, Error> {
        Self::builder(network, client).build().await
    }

    /// Get a builder for custom BoltzSession configuration
    ///
    /// Use this when you need to customize the timeout or provide a specific mnemonic.
    ///
    /// # Example
    /// ```no_run
    /// # use lwk_boltz::BoltzSession;
    /// # use lwk_wollet::ElementsNetwork;
    /// # use lwk_boltz::clients::AnyClient;
    /// # use std::time::Duration;
    /// # async fn example(network: ElementsNetwork, client: AnyClient) -> Result<(), Box<dyn std::error::Error>> {
    /// let session = BoltzSession::builder(network, client)
    ///     .create_swap_timeout(Duration::from_secs(30))
    ///     .build()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder(network: ElementsNetwork, client: AnyClient) -> BoltzSessionBuilder {
        BoltzSessionBuilder::new(network, client)
    }

    /// Internal initialization method that connects to the Boltz API and starts a WebSocket connection
    async fn initialize(
        network: ElementsNetwork,
        client: AnyClient,
        timeout: Option<Duration>,
        mnemonic: Option<Mnemonic>,
        polling: bool,
        timeout_advance: Option<Duration>,
    ) -> Result<Self, Error> {
        let liquid_chain = elements_network_to_liquid_chain(network);
        let chain_client = Arc::new(ChainClient::new().with_liquid(client));
        let url = boltz_default_url(network);
        let api = Arc::new(BoltzApiClientV2::new(url.to_string(), timeout));
        let config = BoltzWsConfig::default();
        let ws_url = url.replace("http", "ws") + "/ws"; // api.get_ws_url() is private
        let ws = Arc::new(BoltzWsApi::new(ws_url, config));
        let future = BoltzWsApi::run_ws_loop(ws.clone());
        tokio::spawn(future); // TODO handle wasm

        let (next_index_to_use, mnemonic) = match mnemonic {
            Some(mnemonic) => (
                fetch_next_index_to_use(&mnemonic, network_kind(liquid_chain), &api).await?,
                mnemonic,
            ),
            None => (0, Mnemonic::generate(12).expect("12 is a valid word count")),
        };
        Ok(Self {
            next_index_to_use: AtomicU32::new(next_index_to_use),
            mnemonic,
            ws,
            api,
            chain_client,
            liquid_chain,
            timeout: timeout.unwrap_or(Duration::from_secs(10)),
            polling,
            timeout_advance: timeout_advance.unwrap_or(Duration::from_secs(180)),
        })
    }

    fn chain(&self) -> Chain {
        Chain::Liquid(self.liquid_chain)
    }

    fn network(&self) -> ElementsNetwork {
        liquid_chain_to_elements_network(self.liquid_chain)
    }

    fn derive_next_keypair(&self) -> Result<(u32, Keypair), Error> {
        let index = self.next_index_to_use.fetch_add(1, Ordering::Relaxed);
        let keypair = derive_keypair(index, &self.mnemonic)?;
        Ok((index, keypair))
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

    /// Fetch all swaps ever done with the session mnemonic from the boltz api.
    ///
    /// This is useful as a swap list but can also be used to restore non-completed swaps that have not
    /// being persisted or that have been lost. TODO: use fn xxx
    pub async fn swap_restore(&self) -> Result<Vec<SwapRestoreResponse>, Error> {
        let xpub = derive_xpub_from_mnemonic(&self.mnemonic, network_kind(self.liquid_chain))?;
        let result = self.api.post_swap_restore(&xpub.to_string()).await?;
        Ok(result)
    }

    /// Fetch information, such as min and max amounts, about the reverse and submarine pairs from the boltz api.
    pub async fn fetch_swaps_info(
        &self,
    ) -> Result<(GetReversePairsResponse, GetSubmarinePairsResponse), Error> {
        let a = self.api.get_reverse_pairs().await?;
        let b = self.api.get_submarine_pairs().await?;
        Ok((a, b))
    }
}

/// Builder for creating a [`BoltzSession`]
pub struct BoltzSessionBuilder {
    network: ElementsNetwork,
    client: AnyClient,
    create_swap_timeout: Option<Duration>,
    mnemonic: Option<Mnemonic>,
    polling: bool,
    timeout_advance: Option<Duration>,
}

impl BoltzSessionBuilder {
    /// Create a new `BoltzSessionBuilder` with required network and client parameters
    pub fn new(network: ElementsNetwork, client: AnyClient) -> Self {
        Self {
            network,
            client,
            create_swap_timeout: None,
            mnemonic: None,
            polling: false,
            timeout_advance: None,
        }
    }

    /// Set the timeout for the Boltz API and WebSocket connection
    ///
    /// If not set, the default timeout of 10 seconds is used.
    pub fn create_swap_timeout(mut self, timeout: Duration) -> Self {
        self.create_swap_timeout = Some(timeout);
        self
    }

    /// Set the timeout for the advance call
    ///
    /// If not set, the default timeout of 3 minutes is used.
    pub fn timeout_advance(mut self, timeout: Duration) -> Self {
        self.timeout_advance = Some(timeout);
        self
    }

    /// Set the mnemonic for deriving swap keys
    ///
    /// If not set, a new random mnemonic will be generated.
    pub fn mnemonic(mut self, mnemonic: Mnemonic) -> Self {
        self.mnemonic = Some(mnemonic);
        self
    }

    /// Set the polling flag
    ///
    /// If true, the advance call will not await on the websocket connection returning immediately
    /// even if there is no update, thus requiring the caller to poll for updates.
    ///
    /// If true, the timeout_advance will be ignored even if set.
    pub fn polling(mut self, polling: bool) -> Self {
        self.polling = polling;
        self
    }

    /// Build the `BoltzSession`
    pub async fn build(self) -> Result<BoltzSession, Error> {
        BoltzSession::initialize(
            self.network,
            self.client,
            self.create_swap_timeout,
            self.mnemonic,
            self.polling,
            self.timeout_advance,
        )
        .await
    }

    /// Build a blocking `BoltzSession`
    ///
    /// This creates a new tokio runtime and wraps the async session for synchronous use.
    pub fn build_blocking(self) -> Result<blocking::BoltzSession, Error> {
        let runtime = Arc::new(tokio::runtime::Runtime::new()?);
        let _guard = runtime.enter();
        let inner = runtime.block_on(self.build())?;
        Ok(blocking::BoltzSession::new_from_async(inner, runtime))
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

pub(crate) fn mnemonic_identifier(mnemonic: &Mnemonic) -> Result<XKeyIdentifier, Error> {
    let seed = mnemonic.to_seed("");
    let xpriv = Xpriv::new_master(NetworkKind::Test, &seed[..])?;
    Ok(xpriv.identifier(&lwk_wollet::EC))
}

async fn fetch_next_index_to_use(
    mnemonic: &Mnemonic,
    network_kind: NetworkKind,
    client: &BoltzApiClientV2,
) -> Result<u32, Error> {
    let xpub = derive_xpub_from_mnemonic(mnemonic, network_kind)?;
    log::info!("xpub for restore is: {}", xpub);

    let result = client.post_swap_restore(&xpub.to_string()).await?;
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
    Ok(next_index_to_use)
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
    network_kind: NetworkKind,
) -> Result<Xpub, Error> {
    let seed = mnemonic.to_seed("");
    let xpriv = Xpriv::new_master(network_kind, &seed[..])?;
    let derivation_path = DerivationPath::master();
    let derived = xpriv.derive_priv(&lwk_wollet::EC, &derivation_path)?;
    Ok(Xpub::from_priv(&lwk_wollet::EC, &derived))
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
    swap_id: &str,
    polling: bool,
) -> Result<SwapStatus, Error> {
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let update = if polling {
            match rx.try_recv() {
                Ok(update) => update,
                Err(TryRecvError::Empty) => {
                    return Err(Error::NoBoltzUpdate);
                }
                Err(e) => return Err(e.into()),
            }
        } else {
            // since we can receive updates for all swaps, we need to check the deadline
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            tokio::select! {
                update = rx.recv() => update?,
                _ = tokio::time::sleep(remaining) => {
                    log::warn!("Timeout while waiting state for swap id {}", swap_id );
                    return Err(Error::Timeout(swap_id.to_string()));
                }
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
        return Ok(update);
    }
}

/// Derive a keypair from a mnemonic and index using the Boltz derivation path
///
/// This derivation path is a constant for Boltz, by using this we are compatible with the web app and can use the same rescue file
fn derive_keypair(index: u32, mnemonic: &Mnemonic) -> Result<Keypair, Error> {
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
    let derived = xpriv.derive_priv(&lwk_wollet::EC, &derivation_path)?;
    log::info!("derive_next_keypair with index: {index}");
    let keypair = Keypair::from_seckey_slice(&lwk_wollet::EC, &derived.private_key.secret_bytes())?;
    Ok(keypair)
}

/// Broadcast a transaction with retry logic
///
/// Attempts to broadcast the transaction up to 30 times, sleeping 1 second between retries on failure.
pub async fn broadcast_tx_with_retry(
    chain_client: &ChainClient,
    tx: &boltz_client::swaps::BtcLikeTransaction,
) -> Result<String, Error> {
    for _ in 0..30 {
        match chain_client.broadcast_tx(tx).await {
            Ok(txid) => return Ok(txid),
            Err(_) => {
                log::info!("Failed broadcast, retrying in 1 second");
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    Err(Error::RetryBroadcastFailed)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bip39::Mnemonic;
    use lightning::offers::offer::Offer;
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
            let chain = crate::elements_network_to_liquid_chain(network);
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
        let network_kind = NetworkKind::Main;
        let xpub = derive_xpub_from_mnemonic(&mnemonic, network_kind).unwrap();
        assert_eq!(xpub.to_string(), expected_xpub);
    }

    #[test]
    fn test_derive_keypair() {
        // from the web app
        let mnemonic = "damp cart merit asset obvious idea chef traffic absent armed road link";
        let expected_keypair_pubkey =
            "0315a98cf1610e96ca92505c6e9536a208353399685440869dca58947a909d07ed";

        let mnemonic: Mnemonic = mnemonic.parse().unwrap();
        let index = 0;
        let keypair = crate::derive_keypair(index, &mnemonic).unwrap();
        assert_eq!(keypair.public_key().to_string(), expected_keypair_pubkey);
    }

    #[test]
    fn test_bolt12() {
        let bolt12_str = "lno1zcss9sy46p548rukhu2vt7g0dsy9r00n2jswepsrngjt7w988ac94hpv";
        let bolt12 = Offer::from_str(bolt12_str).unwrap();
        assert_eq!(bolt12.to_string(), bolt12_str);
    }
}
