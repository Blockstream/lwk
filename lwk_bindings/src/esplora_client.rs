use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lwk_wollet::{
    asyncr,
    clients::blocking::{self, BlockchainBackend},
};

use crate::{BlockHeader, LwkError, Network, Transaction, Txid, Update, Wollet};

/// A blockchain backend implementation based on the
/// [esplora HTTP API](https://github.com/blockstream/esplora/blob/master/API.md)
/// But can also use the [waterfalls](https://github.com/RCasatta/waterfalls) endpoint to
/// speed up the scan if supported by the server.
#[derive(uniffi::Object, Debug)]
pub struct EsploraClient {
    pub(crate) inner: Mutex<blocking::EsploraClient>,

    /// The builder used to create the client, used to create a new client with the same configuration.
    pub(crate) builder: lwk_wollet::clients::EsploraClientBuilder,
}

/// A blockchain backend implementation based on the
/// [Waterfalls HTTP API](https://github.com/RCasatta/waterfalls).
#[derive(uniffi::Object, Debug)]
pub struct WaterfallsClient {
    pub(crate) inner: Mutex<blocking::WaterfallsClient>,
}

/// Provider of a token for authenticated Esplora and Waterfalls backends.
///
/// Some Esplora servers, particularly enterprise deployments like
/// [Blockstream Enterprise](https://blockstream.info/explorer-api), require authentication for
/// access.
#[derive(uniffi::Enum, Clone)]
pub enum TokenProvider {
    /// No token is needed
    None,
    /// A static token is used as-is for every request
    Static {
        /// The token value
        token: String,
    },
    /// An OAuth2 token is obtained from the Blockstream API and refreshed automatically
    Blockstream {
        /// The url to get the token from
        url: String,
        /// The client ID
        client_id: String,
        /// The client secret
        client_secret: String,
    },
}

// Manual `Debug` that redacts secret material (the static token and the
// OAuth client secret) so credentials never leak into logs or error output.
impl std::fmt::Debug for TokenProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenProvider::None => f.write_str("None"),
            TokenProvider::Static { .. } => f
                .debug_struct("Static")
                .field("token", &"<redacted>")
                .finish(),
            TokenProvider::Blockstream { url, client_id, .. } => f
                .debug_struct("Blockstream")
                .field("url", url)
                .field("client_id", client_id)
                .field("client_secret", &"<redacted>")
                .finish(),
        }
    }
}

impl From<TokenProvider> for lwk_wollet::clients::TokenProvider {
    fn from(value: TokenProvider) -> Self {
        match value {
            TokenProvider::None => lwk_wollet::clients::TokenProvider::None,
            TokenProvider::Static { token } => lwk_wollet::clients::TokenProvider::Static(token),
            TokenProvider::Blockstream {
                url,
                client_id,
                client_secret,
            } => lwk_wollet::clients::TokenProvider::Blockstream {
                url,
                client_id,
                client_secret,
            },
        }
    }
}

/// A builder for the `EsploraClient`
#[derive(uniffi::Record)]
pub struct EsploraClientBuilder {
    base_url: String,
    network: Arc<Network>,
    #[uniffi(default = false)]
    waterfalls: bool,
    #[uniffi(default = None)]
    concurrency: Option<u32>,
    #[uniffi(default = None)]
    timeout: Option<u8>,
    #[uniffi(default = false)]
    utxo_only: bool,
    /// HTTP headers to set on each request, for example to authenticate with a backend
    #[uniffi(default = None)]
    headers: Option<HashMap<String, String>>,
    /// Token provider for authenticated Esplora and Waterfalls backends
    #[uniffi(default = None)]
    token_provider: Option<TokenProvider>,
}

/// A builder for the `WaterfallsClient`
#[derive(uniffi::Record)]
pub struct WaterfallsClientBuilder {
    base_url: String,
    network: Arc<Network>,
    #[uniffi(default = None)]
    concurrency: Option<u32>,
    #[uniffi(default = None)]
    timeout: Option<u8>,
    #[uniffi(default = false)]
    utxo_only: bool,
    /// HTTP headers to set on each request, for example to authenticate with a backend
    #[uniffi(default = None)]
    headers: Option<HashMap<String, String>>,
    /// Token provider for authenticated Waterfalls backends
    #[uniffi(default = None)]
    token_provider: Option<TokenProvider>,
}

impl From<EsploraClientBuilder> for lwk_wollet::clients::EsploraClientBuilder {
    #[allow(deprecated)]
    fn from(builder: EsploraClientBuilder) -> Self {
        let mut result = lwk_wollet::clients::EsploraClientBuilder::new(
            &builder.base_url,
            (builder.network.as_ref()).into(),
        );
        if builder.waterfalls {
            result = result.waterfalls(true);
        }
        if let Some(concurrency) = builder.concurrency {
            result = result.concurrency(concurrency as usize);
        }
        if let Some(timeout) = builder.timeout {
            result = result.timeout(timeout);
        }
        if builder.utxo_only {
            result = result.utxo_only(true);
        }
        if let Some(headers) = builder.headers {
            result = result.headers(headers);
        }
        if let Some(token_provider) = builder.token_provider {
            result = result.token_provider(token_provider.into());
        }
        result
    }
}

impl From<WaterfallsClientBuilder> for lwk_wollet::clients::WaterfallsClientBuilder {
    fn from(builder: WaterfallsClientBuilder) -> Self {
        let mut result = lwk_wollet::clients::WaterfallsClientBuilder::new(
            &builder.base_url,
            (builder.network.as_ref()).into(),
        );
        if let Some(concurrency) = builder.concurrency {
            result = result.concurrency(concurrency as usize);
        }
        if let Some(timeout) = builder.timeout {
            result = result.timeout(timeout);
        }
        if builder.utxo_only {
            result = result.utxo_only(true);
        }
        if let Some(headers) = builder.headers {
            result = result.headers(headers);
        }
        if let Some(token_provider) = builder.token_provider {
            result = result.token_provider(token_provider.into());
        }
        result
    }
}

#[uniffi::export]
impl EsploraClient {
    /// Construct an Esplora Client
    #[uniffi::constructor]
    pub fn new(url: &str, network: &Network) -> Result<Arc<Self>, LwkError> {
        let builder = lwk_wollet::clients::EsploraClientBuilder::new(url, network.into());
        let client = builder.clone().build_blocking()?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
            builder,
        }))
    }

    /// Construct an Esplora Client using Waterfalls endpoint
    #[uniffi::constructor]
    pub fn new_waterfalls(url: &str, network: &Network) -> Result<Arc<Self>, LwkError> {
        #[allow(deprecated)]
        let builder =
            lwk_wollet::clients::EsploraClientBuilder::new(url, network.into()).waterfalls(true);
        let client = builder.clone().build_blocking()?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
            builder,
        }))
    }

    /// Construct an Esplora Client from an `EsploraClientBuilder`
    #[uniffi::constructor]
    pub fn from_builder(builder: EsploraClientBuilder) -> Result<Arc<Self>, LwkError> {
        let builder = lwk_wollet::clients::EsploraClientBuilder::from(builder);
        let client = builder.clone().build_blocking()?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
            builder,
        }))
    }

    /// Broadcast a transaction to the network so that a miner can include it in a block.
    pub fn broadcast(&self, tx: &Transaction) -> Result<Arc<Txid>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.broadcast(tx.as_ref())?.into()))
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet
    ///
    /// This method scans both external and internal address chains, stopping after finding
    /// 20 consecutive unused addresses (the gap limit) as recommended by
    /// [BIP44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki#address-gap-limit).
    ///
    /// Returns `Some(Update)` if any changes were found during scanning, or `None` if no changes
    /// were detected.
    ///
    /// To scan beyond the gap limit use `full_scan_to_index()` instead.
    pub fn full_scan(&self, wollet: &Wollet) -> Result<Option<Arc<Update>>, LwkError> {
        self.full_scan_to_index(wollet, 0)
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet up to a specified derivation index
    ///
    /// While `full_scan()` stops after finding 20 consecutive unused addresses (the gap limit),
    /// this method will scan at least up to the given derivation index. This is useful to prevent
    /// missing funds in cases where outputs exist beyond the gap limit.
    ///
    /// Will scan both external and internal address chains up to the given index for maximum safety,
    /// even though internal addresses may not need such deep scanning.
    ///
    /// If transactions are found beyond the gap limit during this scan, subsequent calls to
    /// `full_scan()` will automatically scan up to the highest used index, preventing any
    /// previously-found transactions from being missed.
    pub fn full_scan_to_index(
        &self,
        wollet: &Wollet,
        index: u32,
    ) -> Result<Option<Arc<Update>>, LwkError> {
        let wollet = wollet.inner_wollet()?;
        let update: Option<lwk_wollet::Update> = self
            .inner
            .lock()?
            .full_scan_to_index(&wollet.state(), index)?;
        Ok(update.map(Into::into).map(Arc::new))
    }

    /// See [`BlockchainBackend::tip`]
    pub fn tip(&self) -> Result<Arc<BlockHeader>, LwkError> {
        let tip = self.inner.lock()?.tip()?;
        Ok(Arc::new(tip.into()))
    }
}

#[uniffi::export]
impl WaterfallsClient {
    /// Construct a Waterfalls Client
    #[uniffi::constructor]
    pub fn new(url: &str, network: &Network) -> Result<Arc<Self>, LwkError> {
        let builder = lwk_wollet::clients::WaterfallsClientBuilder::new(url, network.into());
        let client = builder.build_blocking()?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
        }))
    }

    /// Construct a Waterfalls Client from a `WaterfallsClientBuilder`
    #[uniffi::constructor]
    pub fn from_builder(builder: WaterfallsClientBuilder) -> Result<Arc<Self>, LwkError> {
        let builder = lwk_wollet::clients::WaterfallsClientBuilder::from(builder);
        let client = builder.build_blocking()?;
        Ok(Arc::new(Self {
            inner: Mutex::new(client),
        }))
    }

    /// Broadcast a transaction to the network so that a miner can include it in a block.
    pub fn broadcast(&self, tx: &Transaction) -> Result<Arc<Txid>, LwkError> {
        Ok(Arc::new(self.inner.lock()?.broadcast(tx.as_ref())?.into()))
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet
    pub fn full_scan(&self, wollet: &Wollet) -> Result<Option<Arc<Update>>, LwkError> {
        self.full_scan_to_index(wollet, 0)
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet up to a specified derivation index
    pub fn full_scan_to_index(
        &self,
        wollet: &Wollet,
        index: u32,
    ) -> Result<Option<Arc<Update>>, LwkError> {
        let wollet = wollet.inner_wollet()?;
        let update: Option<lwk_wollet::Update> = self
            .inner
            .lock()?
            .full_scan_to_index(&wollet.state(), index)?;
        Ok(update.map(Into::into).map(Arc::new))
    }

    /// See [`BlockchainBackend::tip`]
    pub fn tip(&self) -> Result<Arc<BlockHeader>, LwkError> {
        let tip = self.inner.lock()?.tip()?;
        Ok(Arc::new(tip.into()))
    }
}

impl EsploraClient {
    /// Create a new esplora blocking client with the same connection parameters
    #[allow(unused)] // TODO remove once lwk_boltz is integrated
    pub(crate) fn clone_blocking_client(&self) -> Result<blocking::EsploraClient, LwkError> {
        Ok(self.builder.clone().build_blocking()?)
    }

    /// Create a new esplora async client with the same connection parameters
    #[allow(unused)] // TODO remove once lwk_boltz is integrated
    pub(crate) fn clone_async_client(&self) -> Result<asyncr::EsploraClient, LwkError> {
        Ok(self.builder.clone().build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lwk_wollet::clients::TokenProvider as CoreTokenProvider;

    // These tests exercise the binding-side authentication wiring offline: building a client
    // never performs network I/O because the OAuth token is fetched lazily on the first request.

    #[test]
    fn token_provider_conversion() {
        assert!(matches!(
            CoreTokenProvider::from(TokenProvider::None),
            CoreTokenProvider::None
        ));

        match CoreTokenProvider::from(TokenProvider::Static {
            token: "abc".to_string(),
        }) {
            CoreTokenProvider::Static(token) => assert_eq!(token, "abc"),
            other => panic!("expected Static, got {other:?}"),
        }

        match CoreTokenProvider::from(TokenProvider::Blockstream {
            url: "https://login".to_string(),
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
        }) {
            CoreTokenProvider::Blockstream {
                url,
                client_id,
                client_secret,
            } => {
                assert_eq!(url, "https://login");
                assert_eq!(client_id, "id");
                assert_eq!(client_secret, "secret");
            }
            other => panic!("expected Blockstream, got {other:?}"),
        }
    }

    #[test]
    fn builder_with_auth_builds_offline() {
        let base_url = "https://enterprise.blockstream.info/liquid/api".to_string();

        // Blockstream OAuth2 provider
        let builder = EsploraClientBuilder {
            base_url: base_url.clone(),
            network: Network::mainnet(),
            waterfalls: false,
            concurrency: None,
            timeout: None,
            utxo_only: false,
            headers: None,
            token_provider: Some(TokenProvider::Blockstream {
                url: "https://login".to_string(),
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
            }),
        };
        assert!(EsploraClient::from_builder(builder).is_ok());

        // Static token plus custom headers
        let mut headers = HashMap::new();
        headers.insert("X-Custom".to_string(), "value".to_string());
        let builder = EsploraClientBuilder {
            base_url,
            network: Network::mainnet(),
            waterfalls: false,
            concurrency: None,
            timeout: None,
            utxo_only: false,
            headers: Some(headers),
            token_provider: Some(TokenProvider::Static {
                token: "tok".to_string(),
            }),
        };
        assert!(EsploraClient::from_builder(builder).is_ok());
    }

    #[test]
    fn waterfalls_builder_with_auth_builds_offline() {
        let builder = WaterfallsClientBuilder {
            base_url: "https://enterprise.blockstream.info/liquid/api/waterfalls".to_string(),
            network: Network::mainnet(),
            concurrency: Some(4),
            timeout: None,
            utxo_only: true,
            headers: None,
            token_provider: Some(TokenProvider::Static {
                token: "tok".to_string(),
            }),
        };
        assert!(WaterfallsClient::from_builder(builder).is_ok());
    }
}
