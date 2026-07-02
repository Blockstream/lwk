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

/// A client to issue TCP requests to an electrum server.
pub struct ElectrumClient {
    client: Client,

    tip: BlockHeader,

    script_status: HashMap<Script, ScriptStatus>,
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

        let val = match self.client.script_subscribe(&bitcoin_script) {
            Ok(val) => val,
            Err(electrum_client::Error::AlreadySubscribed(_)) => {
                self.client.script_get_history(&bitcoin_script)?; // it seems it must be called, otherwise the server don't update the status
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
        Ok(self.client.ping()?)
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
    /// Currently only [`TokenProvider::None`] and [`TokenProvider::Static`] are supported
    /// for Electrum; [`TokenProvider::Blockstream`] returns an error from [`Self::build`].
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
        let client = url.build_client_inner(self.timeout, &self.token_provider)?;
        let header = client.block_headers_subscribe_raw()?;
        let tip: BlockHeader = elements_deserialize(&header.header)?;

        Ok(ElectrumClient {
            client,
            tip,
            script_status: HashMap::new(),
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
                if let Ok(header) = self.client.block_headers_subscribe_raw() {
                    let tip: BlockHeader = elements_deserialize(&header.header)?;
                    self.tip = tip;
                }
            }
        }

        Ok(self.tip.clone())
    }

    fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error> {
        // TODO: check that the transaction contains some signatures

        let txid = self
            .client
            .transaction_broadcast_raw(&elements_serialize(tx))?;
        Ok(Txid::from_raw_hash(txid.to_raw_hash()))
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<Transaction>, Error> {
        let txids: Vec<bitcoin::Txid> = txids
            .iter()
            .map(|t| bitcoin::Txid::from_raw_hash(t.to_raw_hash()))
            .collect();

        let mut result = vec![];
        for tx in self.client.batch_transaction_get_raw(&txids)? {
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
        for header in self.client.batch_block_header_raw(heights)? {
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
            .client
            .batch_script_get_history(&scripts)?
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
            &TokenProvider::None,
        )
    }

    /// Build an electrum-client [`Client`] from the url, timeout and token provider.
    pub(crate) fn build_client_inner(
        &self,
        timeout: Option<Duration>,
        token_provider: &TokenProvider,
    ) -> Result<Client, Error> {
        let builder = ConfigBuilder::new();
        let (url, builder) = match self {
            ElectrumUrl::Tls(url, validate) => {
                (format!("ssl://{url}"), builder.validate_domain(*validate))
            }
            ElectrumUrl::Plaintext(url) => (format!("tcp://{url}"), builder),
        };
        let builder = builder
            .timeout(timeout)
            .authorization_provider(token_provider_auth(token_provider)?);
        Ok(Client::from_config(&url, builder.build())?)
    }
}

/// Convert a [`TokenProvider`] into an electrum-client [`AuthProvider`] (a closure that
/// returns the `authorization` header value). Only `None`/`Static` are supported for
/// Electrum; `Blockstream` requires fetching a token and is not wired here yet.
fn token_provider_auth(token_provider: &TokenProvider) -> Result<Option<AuthProvider>, Error> {
    match token_provider {
        TokenProvider::None => Ok(None),
        TokenProvider::Static(token) => {
            let header = format!("Bearer {token}");
            Ok(Some(Arc::new(move || Some(header.clone())) as AuthProvider))
        }
        TokenProvider::Blockstream { .. } => Err(Error::Generic(
            "TokenProvider::Blockstream is not yet supported for the Electrum client; use TokenProvider::Static".to_string(),
        )),
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

        assert!(token_provider_auth(&TokenProvider::None).unwrap().is_none());

        let provider = token_provider_auth(&TokenProvider::Static("tok".to_string()))
            .unwrap()
            .expect("a static token yields an auth provider");
        assert_eq!(provider(), Some("Bearer tok".to_string()));

        assert!(
            token_provider_auth(&TokenProvider::Blockstream {
                url: "https://example/token".to_string(),
                client_id: "id".to_string(),
                client_secret: "secret".to_string(),
            })
            .is_err(),
            "Blockstream is not supported for Electrum yet and should error"
        );
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
