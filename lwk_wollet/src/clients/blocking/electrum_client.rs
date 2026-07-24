use crate::cache::Height;
use crate::clients::electrum_url::ElectrumUrl;
use crate::clients::TokenProvider;
use crate::Error;
use crate::History;

use electrum_client::ScriptStatus;
use electrum_client::{AuthProvider, Client, ConfigBuilder, ElectrumApi, GetHistoryRes};
use elements::encode::deserialize as elements_deserialize;
use elements::encode::serialize as elements_serialize;
use elements::Address;
use elements::{bitcoin, BlockHash, BlockHeader, Script, Transaction, Txid};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use super::BlockchainBackend;

/// Shared cache of the OAuth token minted by the [`AuthProvider`] closure, so the client
/// can invalidate it when the server denies a call with an authentication error.
#[cfg(feature = "electrum_oidc")]
type AuthTokenCache = Arc<std::sync::Mutex<Option<String>>>;

/// A client to issue TCP requests to an electrum server.
pub struct ElectrumClient {
    client: Client,

    tip: BlockHeader,

    script_status: HashMap<Script, ScriptStatus>,

    /// For [`TokenProvider::Blockstream`]: the token cache shared with the [`AuthProvider`]
    /// closure, so an authentication denial can invalidate the token and the retried call
    /// mints a fresh one. `None` for the other providers.
    #[cfg(feature = "electrum_oidc")]
    token_cache: Option<AuthTokenCache>,
}

impl Debug for ElectrumClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElectrumClient")
            .field("tip", &self.tip)
            .finish()
    }
}

/// Options for the deprecated [`ElectrumClient::with_options()`] method.
///
/// Use [`ElectrumClientBuilder`] instead.
#[derive(Default)]
#[deprecated(note = "use ElectrumClientBuilder instead")]
pub struct ElectrumOptions {
    /// The timeout for the Electrum client.
    pub timeout: Option<u8>,
}

impl ElectrumClient {
    /// Creates an Electrum client with default options. To set a timeout or a token
    /// provider, use [`ElectrumClientBuilder`].
    // TODO: deprecate in favour of ElectrumClientBuilder.
    pub fn new(url: &ElectrumUrl) -> Result<Self, Error> {
        ElectrumClientBuilder::new(&url.to_string()).build()
    }

    /// Creates an Electrum client specifying non default options like timeout.
    #[deprecated(note = "use ElectrumClientBuilder instead")]
    #[allow(deprecated)]
    pub fn with_options(url: &ElectrumUrl, options: ElectrumOptions) -> Result<Self, Error> {
        let mut builder = ElectrumClientBuilder::new(&url.to_string());
        if let Some(timeout) = options.timeout {
            builder = builder.timeout(Duration::from_secs(timeout as u64));
        }
        builder.build()
    }

    /// Return the status of an address as defined by the electrum protocol
    ///
    /// The status is function of the transaction ids where this address appears and the height of
    /// the block containing when it is confirmed. Unconfirmed transactions use a negative height,
    /// so the status change when they are confirmed.
    pub fn address_status(&mut self, address: &Address) -> Result<Option<ScriptStatus>, Error> {
        let elements_script = address.script_pubkey();
        let bitcoin_script = bitcoin::ScriptBuf::from(elements_script.to_bytes());

        let val = match self.with_token_refresh(|client| client.script_subscribe(&bitcoin_script)) {
            Ok(val) => val,
            Err(electrum_client::Error::AlreadySubscribed(_)) => {
                self.with_token_refresh(|client| client.script_get_history(&bitcoin_script))?; // it seems it must be called, otherwise the server don't update the status
                self.client.script_pop(&bitcoin_script)?
            }
            Err(e) => return Err(e.into()),
        };

        if let Some(val) = val {
            self.script_status.insert(elements_script.clone(), val);
        }
        Ok(self.script_status.get(&elements_script).cloned())
    }

    /// Ping the Electrum server
    pub fn ping(&self) -> Result<(), Error> {
        Ok(self.with_token_refresh(|client| client.ping())?)
    }

    /// Run an electrum call and, when the server denies it with an authentication error
    /// (the cached OAuth token expired and the connection was re-established), invalidate
    /// the cached token and retry the call once.
    ///
    /// The retry works because the server closes the denied connection: the retried call
    /// hits the dead connection, so electrum-client transparently reconnects and the
    /// [`AuthProvider`] closure mints a fresh token for the new connection.
    ///
    /// Without the `electrum_oidc` feature (or with a non-`Blockstream` provider) the call
    /// runs exactly once.
    fn with_token_refresh<T>(
        &self,
        mut op: impl FnMut(&Client) -> Result<T, electrum_client::Error>,
    ) -> Result<T, electrum_client::Error> {
        let result = op(&self.client);
        #[cfg(feature = "electrum_oidc")]
        if let (Err(e), Some(cache)) = (&result, &self.token_cache) {
            if is_auth_denied(e) {
                log::debug!("authentication denied, invalidating the token and retrying once");
                if let Ok(mut guard) = cache.lock() {
                    *guard = None;
                }
                return op(&self.client);
            }
        }
        result
    }
}

/// Whether the error is the authenticated Electrum RPC proxy denying the call because it
/// lacks a valid token (JSON-RPC code -32004, AUTHENTICATION_REQUIRED).
///
/// The denial can arrive nested inside [`electrum_client::Error::AllAttemptsErrored`]: when a
/// connection drops (e.g. the token expired mid-session and the server closed the idle
/// connection), electrum-client reconnects and re-runs the `server.version` handshake, which
/// carries the token and is denied with -32004. If the preceding reconnect attempt already
/// spent the retry budget, that -32004 is wrapped in `AllAttemptsErrored` rather than surfaced
/// directly, so this checks the wrapped errors too.
#[cfg(feature = "electrum_oidc")]
fn is_auth_denied(error: &electrum_client::Error) -> bool {
    // TODO: the proxy JSON-RPC denial codes (-32004 auth, -32000 credits, ...) and this
    // check are duplicated here, in lwk_test_util, and the e2e tests. Share them (exporting
    // from the proxy crate is awkward, so at least a shared set of constants in lwk) so they
    // stay in sync.
    const AUTHENTICATION_REQUIRED: i64 = -32004;
    match error {
        electrum_client::Error::Protocol(value) => {
            value.get("code").and_then(|c| c.as_i64()) == Some(AUTHENTICATION_REQUIRED)
        }
        electrum_client::Error::AllAttemptsErrored(errors) => errors.iter().any(is_auth_denied),
        _ => false,
    }
}

/// Builder for an [`ElectrumClient`].
#[derive(Debug, Clone)]
pub struct ElectrumClientBuilder {
    url: String,
    timeout: Option<Duration>,
    token_provider: TokenProvider,
    allow_plaintext_with_token: bool,
}

impl ElectrumClientBuilder {
    /// Create a new builder for the given Electrum `url`, e.g. `ssl://example.com:50002`
    /// or `tcp://example.com:50001`. The url is parsed when [`Self::build`] is called.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            timeout: None,
            token_provider: TokenProvider::None,
            allow_plaintext_with_token: false,
        }
    }

    /// Set the timeout for the Electrum client connection and requests.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the token provider used to authenticate to the Electrum server (e.g. a JWT
    /// `Bearer` token injected as the `authorization` member of the JSON-RPC requests,
    /// for proxies that require it).
    ///
    /// [`TokenProvider::None`] and [`TokenProvider::Static`] are always supported.
    /// [`TokenProvider::Blockstream`] (automatic OAuth2 token fetch and refresh) requires
    /// the `electrum_oidc` cargo feature; without it, [`Self::build`] returns an error.
    ///
    /// With `TokenProvider::Blockstream` the token is fetched by [`Self::build`] and
    /// cached. The server validates the token when a connection is established, so an
    /// expired token surfaces as an authentication denial (JSON-RPC error -32004) on the
    /// first call after a reconnection: the client then invalidates the cached token and
    /// retries the call once, minting a fresh token for the new connection.
    ///
    /// Security:
    /// - The token is only protected in transit on TLS (`ssl://`) connections. Setting a
    ///   token on a plaintext (`tcp://`) connection makes [`Self::build`] return an error,
    ///   since the token would be sent in cleartext; opt in with
    ///   [`Self::allow_plaintext_with_token`] if that is really intended (e.g. a localhost
    ///   proxy or a connection already tunneled).
    /// - electrum-client logs full JSON-RPC requests at `TRACE` level, which includes the
    ///   `authorization` token. Avoid enabling `TRACE` logging for the `electrum_client`
    ///   target when using a sensitive token (redaction tracked upstream:
    ///   bitcoindevkit/rust-electrum-client#215).
    pub fn token_provider(mut self, token_provider: TokenProvider) -> Self {
        self.token_provider = token_provider;
        self
    }

    /// Allow sending the token over a plaintext (`tcp://`) connection.
    ///
    /// By default [`Self::build`] errors if a token is set on a plaintext connection,
    /// because the token would travel in cleartext. Set this to `true` only when that is
    /// intended (e.g. a localhost proxy, or a connection already tunneled/encrypted).
    pub fn allow_plaintext_with_token(mut self, allow: bool) -> Self {
        self.allow_plaintext_with_token = allow;
        self
    }

    /// Build the [`ElectrumClient`], opening the connection.
    pub fn build(self) -> Result<ElectrumClient, Error> {
        let url: ElectrumUrl = self.url.parse()?;
        if matches!(url, ElectrumUrl::Plaintext(_))
            && !matches!(self.token_provider, TokenProvider::None)
            && !self.allow_plaintext_with_token
        {
            return Err(Error::Generic(
                "refusing to send an Electrum auth token over a plaintext (tcp://) connection; use ssl:// or call allow_plaintext_with_token(true)".to_string(),
            ));
        }
        let auth = token_provider_auth(&self.token_provider)?;
        let client = url.build_client_inner(self.timeout, auth.provider, auth.retry)?;
        let header = client.block_headers_subscribe_raw()?;
        let tip: BlockHeader = elements_deserialize(&header.header)?;

        Ok(ElectrumClient {
            client,
            tip,
            script_status: HashMap::new(),
            #[cfg(feature = "electrum_oidc")]
            token_cache: auth.token_cache,
        })
    }
}

impl BlockchainBackend for ElectrumClient {
    fn tip(&mut self) -> Result<BlockHeader, Error> {
        let mut popped_header = None;
        while let Some(header) = self.client.block_headers_pop_raw()? {
            popped_header = Some(header)
        }

        match popped_header {
            Some(header) => {
                let tip: BlockHeader = elements_deserialize(&header.header)?;
                self.tip = tip;
            }
            None => {
                // https://github.com/bitcoindevkit/rust-electrum-client/issues/124
                // It might be that the client has reconnected and subscriptions don't persist
                // across connections. Calling `client.ping()` won't help here because the
                // successful retry will prevent us knowing about the reconnect.
                if let Ok(header) =
                    self.with_token_refresh(|client| client.block_headers_subscribe_raw())
                {
                    let tip: BlockHeader = elements_deserialize(&header.header)?;
                    self.tip = tip;
                }
            }
        }

        Ok(self.tip.clone())
    }

    fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error> {
        // TODO: check that the transaction contains some signatures

        let tx_bytes = elements_serialize(tx);
        let txid = self.with_token_refresh(|client| client.transaction_broadcast_raw(&tx_bytes))?;
        Ok(Txid::from_raw_hash(txid.to_raw_hash()))
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<Transaction>, Error> {
        let txids: Vec<bitcoin::Txid> = txids
            .iter()
            .map(|t| bitcoin::Txid::from_raw_hash(t.to_raw_hash()))
            .collect();

        let mut result = vec![];
        for tx in self.with_token_refresh(|client| client.batch_transaction_get_raw(&txids))? {
            let tx: Transaction = elements::encode::deserialize(&tx)?;
            result.push(tx);
        }
        Ok(result)
    }

    fn get_headers(
        &self,
        heights: &[Height],
        _: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<BlockHeader>, Error> {
        let mut result = vec![];
        for header in self.with_token_refresh(|client| client.batch_block_header_raw(heights))? {
            let header: BlockHeader = elements::encode::deserialize(&header)?;
            result.push(header);
        }
        Ok(result)
    }

    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        let scripts: Vec<&bitcoin::Script> = scripts
            .iter()
            .map(|t| bitcoin::Script::from_bytes(t.as_bytes()))
            .collect();

        Ok(self
            .with_token_refresh(|client| client.batch_script_get_history(&scripts))?
            .into_iter()
            .map(|e| e.into_iter().map(Into::into).collect())
            .collect())
    }
}

impl From<GetHistoryRes> for History {
    fn from(value: GetHistoryRes) -> Self {
        History {
            txid: Txid::from_raw_hash(value.tx_hash.to_raw_hash()),
            height: value.height,
            block_hash: None,
            block_timestamp: None,
            v: 0,
        }
    }
}

impl ElectrumUrl {
    /// Build an Electrum client from the url and options.
    #[deprecated(note = "use ElectrumClientBuilder instead")]
    #[allow(deprecated)]
    pub fn build_client(&self, options: &ElectrumOptions) -> Result<Client, Error> {
        self.build_client_inner(
            options.timeout.map(|t| Duration::from_secs(t as u64)),
            None,
            None,
        )
    }

    /// Build an electrum-client [`Client`] from the url, timeout, authorization provider and
    /// optional `retry` override (`None` keeps electrum-client's default).
    pub(crate) fn build_client_inner(
        &self,
        timeout: Option<Duration>,
        auth_provider: Option<AuthProvider>,
        retry: Option<u8>,
    ) -> Result<Client, Error> {
        let builder = ConfigBuilder::new();
        let (url, builder) = match self {
            ElectrumUrl::Tls(url, validate) => {
                (format!("ssl://{url}"), builder.validate_domain(*validate))
            }
            ElectrumUrl::Plaintext(url) => (format!("tcp://{url}"), builder),
        };
        let mut builder = builder
            .timeout(timeout)
            .authorization_provider(auth_provider);
        if let Some(retry) = retry {
            builder = builder.retry(retry);
        }
        Ok(Client::from_config(&url, builder.build())?)
    }
}

/// The result of converting a [`TokenProvider`]: the electrum-client [`AuthProvider`]
/// closure plus, for [`TokenProvider::Blockstream`], the token cache shared with it and the
/// retry override needed for the reactive refresh.
struct ElectrumAuth {
    provider: Option<AuthProvider>,
    /// electrum-client `retry` override (`None` keeps its default of 1).
    retry: Option<u8>,
    #[cfg(feature = "electrum_oidc")]
    token_cache: Option<AuthTokenCache>,
}

impl ElectrumAuth {
    fn new(provider: Option<AuthProvider>) -> Self {
        Self {
            provider,
            retry: None,
            #[cfg(feature = "electrum_oidc")]
            token_cache: None,
        }
    }
}

/// Convert a [`TokenProvider`] into an electrum-client [`AuthProvider`] (a closure that
/// returns the `authorization` header value).
///
/// For [`TokenProvider::Blockstream`] (feature `electrum_oidc`) this fetches the first
/// token, so failures like wrong credentials surface here with their real cause instead
/// of an authentication denial from the server.
fn token_provider_auth(token_provider: &TokenProvider) -> Result<ElectrumAuth, Error> {
    match token_provider {
        TokenProvider::None => Ok(ElectrumAuth::new(None)),
        TokenProvider::Static(token) => {
            let header = format!("Bearer {token}");
            Ok(ElectrumAuth::new(Some(
                Arc::new(move || Some(header.clone())) as AuthProvider,
            )))
        }
        TokenProvider::Blockstream {
            url,
            client_id,
            client_secret,
        } => {
            #[cfg(not(feature = "electrum_oidc"))]
            {
                let _ = (url, client_id, client_secret);
                Err(Error::Generic(
                    "TokenProvider::Blockstream for the Electrum client requires the `electrum_oidc` cargo feature; enable it or use TokenProvider::Static".to_string(),
                ))
            }
            #[cfg(feature = "electrum_oidc")]
            {
                use crate::clients::oauth::fetch_oauth_token_blocking;

                // Mint the first token now: the connection's first message needs it anyway.
                let token = fetch_oauth_token_blocking(url, client_id, client_secret)?;
                let token_cache: AuthTokenCache = Arc::new(std::sync::Mutex::new(Some(token)));

                let cache = token_cache.clone();
                let url = url.clone();
                let client_id = client_id.clone();
                let client_secret = client_secret.clone();
                // Invoked by electrum-client before each request: returns the cached token,
                // minting a new one after the client invalidated it on an authentication
                // denial. If the mint fails the request is sent without a token, so the
                // server's denial surfaces to the caller (the failure itself is only logged).
                let provider = Arc::new(move || {
                    let mut guard = cache.lock().ok()?;
                    if guard.is_none() {
                        log::debug!("fetching authentication token");
                        match fetch_oauth_token_blocking(&url, &client_id, &client_secret) {
                            Ok(token) => *guard = Some(token),
                            Err(e) => {
                                log::warn!("failed to fetch the authentication token: e='{e}'")
                            }
                        }
                    }
                    guard.as_ref().map(|token| format!("Bearer {token}"))
                }) as AuthProvider;

                Ok(ElectrumAuth {
                    provider: Some(provider),
                    // electrum-client drops the reconnect's error when its retry budget is
                    // exhausted (returning `AllAttemptsErrored` without it), so the -32004
                    // from the reconnect's `server.version` handshake is only surfaced with
                    // retry >= 2: one attempt is spent on the dead connection, the next
                    // carries the -32004.
                    // TODO: drop this workaround (back to the default retry) once the upstream
                    // fix lands: https://github.com/bitcoindevkit/rust-electrum-client/issues/221
                    retry: Some(2),
                    token_cache: Some(token_cache),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ElectrumUrl;
    use crate::UrlError;

    fn check_url(url: &str, url_no_scheme: &str, tls: bool, validate_domain: bool) {
        let electrum_url: ElectrumUrl = url.parse().unwrap();
        let url_from_new = ElectrumUrl::new(url_no_scheme, tls, validate_domain).unwrap();
        assert_eq!(electrum_url, url_from_new);
        assert_eq!(electrum_url.to_string(), url);
    }

    #[test]
    fn test_electrum_url() {
        check_url(
            "ssl://blockstream.info:666",
            "blockstream.info:666",
            true,
            true,
        );

        check_url(
            "tcp://blockstream.info:666",
            "blockstream.info:666",
            false,
            false,
        );

        check_url("tcp://1.1.1.1:666", "1.1.1.1:666", false, false);

        check_url(
            "tcp://mrrxtq6tjpbnbm7vh5jt6mpjctn7ggyfy5wegvbeff3x7jrznqawlmid.onion:666",
            "mrrxtq6tjpbnbm7vh5jt6mpjctn7ggyfy5wegvbeff3x7jrznqawlmid.onion:666",
            false,
            false,
        );

        let url_result: Result<ElectrumUrl, UrlError> = "ssl://1.1.1.1:666".parse();
        assert_eq!(
            url_result.unwrap_err().to_string(),
            "Cannot specify `ssl` scheme without a domain"
        );

        let url_result: Result<ElectrumUrl, UrlError> = "http://blockstream.info".parse();
        assert_eq!(
            url_result.unwrap_err().to_string(),
            "Invalid schema `http` supported ones are `ssl` or `tcp`"
        );

        let url_result: Result<ElectrumUrl, UrlError> = "tcp://blockstream.info".parse();
        assert_eq!(url_result.unwrap_err().to_string(), "Port is missing");

        let url_result: Result<ElectrumUrl, UrlError> = "mailto:rms@example.net".parse();
        assert_eq!(
            url_result.unwrap_err().to_string(),
            "Invalid schema `mailto` supported ones are `ssl` or `tcp`"
        );

        let url_result: Result<ElectrumUrl, UrlError> = "xxx".parse();
        assert_eq!(
            url_result.unwrap_err().to_string(),
            "relative URL without a base"
        );
    }

    #[test]
    fn test_electrum_url_new() {
        let err = ElectrumUrl::new("example.com", false, true)
            .unwrap_err()
            .to_string();
        assert_eq!(err, "Cannot validate the domain without tls");

        let err = ElectrumUrl::new("ssl://example.com", false, false)
            .unwrap_err()
            .to_string();
        assert_eq!(err, "Don't specify the scheme in the url");
    }

    #[test]
    fn test_client_connection_is_established_on_build() {
        use electrum_client::{Client, ConfigBuilder};

        // Use a hostname that definitely does not exist to avoid any chance of connection
        let url = "tcp://this-host-definitely-does-not-exist.example.com:50001";
        let config = ConfigBuilder::new()
            .timeout(Some(std::time::Duration::from_secs(1))) // Short timeout to make the test faster
            .build();

        // Building the client should return an error because we cannot resolve the host.
        // This shows that the connection attempt (to resolve the host and establish TCP connection)
        // happens during `Client::from_config`, i.e., when building the client.
        let result = Client::from_config(url, config);
        assert!(
            result.is_err(),
            "Expected an error when trying to build a client with a non-existent host, indicating that the connection is established on build"
        );
    }

    #[test]
    fn token_provider_auth_maps_to_bearer() {
        use super::token_provider_auth;
        use crate::clients::TokenProvider;

        assert!(token_provider_auth(&TokenProvider::None)
            .unwrap()
            .provider
            .is_none());

        let provider = token_provider_auth(&TokenProvider::Static("tok".to_string()))
            .unwrap()
            .provider
            .expect("a static token yields an auth provider");
        assert_eq!(provider(), Some("Bearer tok".to_string()));

        #[cfg(not(feature = "electrum_oidc"))]
        assert!(
            token_provider_auth(&TokenProvider::Blockstream {
                url: "https://example/token".to_string(),
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
            })
            .is_err(),
            "Blockstream requires the electrum_oidc feature and should error without it"
        );
    }

    /// The `Blockstream` auth provider mints a token when created, serves it from the
    /// cache on subsequent calls, and mints a fresh one after the cache is invalidated
    /// (what the client does when the server denies a call with -32004).
    #[cfg(feature = "electrum_oidc")]
    #[test]
    fn blockstream_token_is_minted_cached_and_refreshed() {
        use super::token_provider_auth;
        use crate::clients::TokenProvider;
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        // Minimal OAuth token endpoint: each request is counted and answered with a new
        // access_token (tok1, tok2, ...).
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let hits = Arc::new(AtomicUsize::new(0));
        let server_hits = hits.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let n = server_hits.fetch_add(1, Ordering::SeqCst) + 1;
                // Read until the whole request (headers + form body) is in: the request
                // is tiny, but it may still arrive split across reads.
                let mut request = Vec::new();
                let mut buf = [0u8; 1024];
                loop {
                    let read = stream.read(&mut buf).unwrap();
                    request.extend_from_slice(&buf[..read]);
                    let text = String::from_utf8_lossy(&request);
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let content_length: usize = head
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(str::trim)
                                    .map(str::to_string)
                            })
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                        if body.len() >= content_length {
                            break;
                        }
                    }
                }
                let body = format!(r#"{{"access_token":"tok{n}"}}"#);
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len(),
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        let auth = token_provider_auth(&TokenProvider::Blockstream {
            url: format!("http://127.0.0.1:{port}/token"),
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
        })
        .unwrap();
        let provider = auth.provider.expect("Blockstream yields an auth provider");
        let cache = auth.token_cache.expect("Blockstream yields a token cache");

        // the first token is minted when the provider is created
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // calls serve the cached token without touching the endpoint
        assert_eq!(provider(), Some("Bearer tok1".to_string()));
        assert_eq!(provider(), Some("Bearer tok1".to_string()));
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // invalidating the cache (as the client does on an authentication denial)
        // makes the next call mint a fresh token
        cache.lock().unwrap().take();
        assert_eq!(provider(), Some("Bearer tok2".to_string()));
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    /// Only the proxy's AUTHENTICATION_REQUIRED denial triggers the token refresh; other
    /// denials (e.g. insufficient credits) and other errors are surfaced untouched.
    #[cfg(feature = "electrum_oidc")]
    #[test]
    fn only_authentication_denials_are_retried() {
        use super::is_auth_denied;

        let auth = electrum_client::Error::Protocol(
            serde_json::json!({"code": -32004, "message": "authentication required"}),
        );
        assert!(is_auth_denied(&auth));

        let credits = electrum_client::Error::Protocol(
            serde_json::json!({"code": -32000, "message": "insufficient credits"}),
        );
        assert!(!is_auth_denied(&credits));

        let other = electrum_client::Error::Message("boom".to_string());
        assert!(!is_auth_denied(&other));

        // A -32004 nested in AllAttemptsErrored (the reconnect-after-drop case) is detected.
        let nested = electrum_client::Error::AllAttemptsErrored(vec![
            electrum_client::Error::Message("Broken pipe".to_string()),
            electrum_client::Error::Protocol(
                serde_json::json!({"code": -32004, "message": "authentication required"}),
            ),
        ]);
        assert!(is_auth_denied(&nested));

        // AllAttemptsErrored without a -32004 is not treated as an auth denial.
        let nested_io = electrum_client::Error::AllAttemptsErrored(vec![
            electrum_client::Error::Message("Broken pipe".to_string()),
            electrum_client::Error::Message("unexpected EOF".to_string()),
        ]);
        assert!(!is_auth_denied(&nested_io));
    }

    #[test]
    fn authorization_is_sent_on_the_first_message() {
        use super::ElectrumClientBuilder;
        use crate::clients::TokenProvider;
        use std::io::{BufRead, BufReader};
        use std::net::TcpListener;
        use std::time::Duration;

        // Spin up a throwaway TCP server, build a client against it, and capture the first
        // line the client sends (the `server.version` handshake). The build errors once the
        // mock closes without replying — we only care about what went on the wire.
        fn first_message(token_provider: TokenProvider) -> String {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream).read_line(&mut line).unwrap();
                line
            });

            let _ = ElectrumClientBuilder::new(&format!("tcp://127.0.0.1:{port}"))
                .timeout(Duration::from_secs(5))
                .token_provider(token_provider)
                .allow_plaintext_with_token(true)
                .build();

            server.join().unwrap()
        }

        let with_token = first_message(TokenProvider::Static("test-token".to_string()));
        assert!(
            with_token.contains(r#""authorization":"Bearer test-token""#),
            "expected the bearer token on the first message, got: {with_token}"
        );

        let without_token = first_message(TokenProvider::None);
        assert!(
            !without_token.contains("authorization"),
            "expected no authorization field without a token, got: {without_token}"
        );
    }

    #[test]
    fn plaintext_with_token_errors_without_optin() {
        use super::ElectrumClientBuilder;
        use crate::clients::TokenProvider;

        // A token on a plaintext (tcp://) connection is refused unless explicitly allowed,
        // so this fails fast at build() without attempting any connection.
        let err = ElectrumClientBuilder::new("tcp://127.0.0.1:1")
            .token_provider(TokenProvider::Static("tok".to_string()))
            .build();
        assert!(
            err.is_err(),
            "expected an error when a token is set on a plaintext connection without opt-in"
        );
    }
}
