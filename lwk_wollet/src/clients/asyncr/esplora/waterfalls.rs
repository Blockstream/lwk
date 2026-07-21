use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    collections::{HashSet, VecDeque},
    str,
};

use age::x25519::Recipient;
use elements::{BlockHash, Script, Txid};
#[cfg(not(target_arch = "wasm32"))]
use reqwest::Response;

use crate::{
    cache::Height,
    clients::{waterfalls::waterfalls_subscribe_url, History, WaterfallsClientBuilder},
    Error, Network, Update, Wollet, WolletDescriptor,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    clients::{
        waterfalls::WaterfallsSseParser, Capability, Data, WaterfallsSubscriptionEvent,
        WaterfallsSubscriptionUpdate,
    },
    wollet::WolletState,
};

#[cfg(not(target_arch = "wasm32"))]
use super::{error_for_status, EsploraClient, LastUsedIndexResponse};
#[cfg(target_arch = "wasm32")]
use super::{EsploraClient, LastUsedIndexResponse};

/// A Waterfalls descriptor subscription stream.
///
/// The stream yields typed Waterfalls update events. A `Tip` event only means
/// the chain tip changed; other event kinds are wallet invalidation hints.
#[cfg(not(target_arch = "wasm32"))]
pub struct WaterfallsSubscription {
    response: Response,
    parser: WaterfallsSseParser,
    pending: VecDeque<WaterfallsSubscriptionEvent>,
}

/// A Waterfalls descriptor subscription stream that can reopen itself.
///
/// The stream emits local lifecycle updates when the underlying SSE connection
/// closes and when it is reopened. Callers should run a normal scan after a
/// reconnect because updates may have been missed while disconnected.
#[cfg(not(target_arch = "wasm32"))]
pub struct WaterfallsReconnectingSubscription {
    client: WaterfallsClient,
    url: String,
    subscription: Option<WaterfallsSubscription>,
}

#[cfg(not(target_arch = "wasm32"))]
impl WaterfallsSubscription {
    /// Return the next Waterfalls subscription update.
    ///
    /// Returns `Ok(None)` when the server closes the stream.
    pub async fn next_update(&mut self) -> Result<Option<WaterfallsSubscriptionEvent>, Error> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }

            let Some(chunk) = self.response.chunk().await? else {
                return Ok(None);
            };
            let chunk = str::from_utf8(&chunk)
                .map_err(|e| Error::Generic(format!("invalid Waterfalls SSE UTF-8: {e}")))?;
            self.pending.extend(self.parser.push_str(chunk)?);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl WaterfallsReconnectingSubscription {
    /// Return the next reconnecting Waterfalls subscription update.
    pub async fn next_update(&mut self) -> Result<WaterfallsSubscriptionUpdate, Error> {
        let Some(subscription) = self.subscription.as_mut() else {
            self.subscription = Some(self.client.subscribe_url(&self.url).await?);
            return Ok(WaterfallsSubscriptionUpdate::Reconnected);
        };

        match subscription.next_update().await {
            Ok(Some(event)) => Ok(WaterfallsSubscriptionUpdate::Event(event)),
            Ok(None) => {
                self.subscription = None;
                Ok(WaterfallsSubscriptionUpdate::Disconnected { error: None })
            }
            Err(err) => {
                self.subscription = None;
                Ok(WaterfallsSubscriptionUpdate::Disconnected {
                    error: Some(err.to_string()),
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
/// A blockchain backend implementation based on the
/// [Waterfalls HTTP API](https://github.com/RCasatta/waterfalls).
///
/// Waterfalls is Esplora-compatible for common chain operations and adds
/// descriptor-based wallet scan endpoints.
pub struct WaterfallsClient {
    inner: EsploraClient,
}

impl WaterfallsClient {
    /// Creates a new Waterfalls client with default options using the given `url` as endpoint.
    ///
    /// To specify different options use the [`WaterfallsClientBuilder`].
    pub fn new(network: Network, url: &str) -> Self {
        WaterfallsClientBuilder::new(url, network)
            .build()
            .expect("cannot fail with this configuration")
    }

    /// Async version of [`crate::blocking::BlockchainBackend::tip()`].
    pub async fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        self.inner.tip().await
    }

    /// Async version of [`crate::blocking::BlockchainBackend::broadcast()`].
    pub async fn broadcast(
        &self,
        tx: &elements::Transaction,
    ) -> Result<elements::Txid, crate::Error> {
        self.inner.broadcast(tx).await
    }

    /// Fetch a transaction.
    pub async fn get_transaction(&self, txid: Txid) -> Result<elements::Transaction, Error> {
        self.inner.get_transaction(txid).await
    }

    /// Fetch concurrently a list of transactions.
    pub async fn get_transactions(
        &self,
        txids: &[Txid],
    ) -> Result<Vec<elements::Transaction>, Error> {
        self.inner.get_transactions(txids).await
    }

    /// Fetch concurrently a list of block headers.
    ///
    /// Optionally pass known blockhash to avoid some network roundtrips if already known.
    pub async fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        self.inner.get_headers(heights, height_blockhash).await
    }

    /// Get the transactions involved in a list of scripts.
    pub async fn get_scripts_history(
        &self,
        scripts: &[&Script],
    ) -> Result<Vec<Vec<History>>, Error> {
        self.inner.get_scripts_history(scripts).await
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet.
    pub async fn full_scan(&mut self, wollet: &Wollet) -> Result<Option<Update>, Error> {
        self.inner.full_scan(wollet).await
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet up to a specified derivation index.
    pub async fn full_scan_to_index(
        &mut self,
        wollet: &Wollet,
        index: u32,
    ) -> Result<Option<Update>, Error> {
        self.inner.full_scan_to_index(wollet, index).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn subscribe_url(&self, url: &str) -> Result<WaterfallsSubscription, Error> {
        let response = self.inner.get_with_retry(url).await?;
        if !response.status().is_success() {
            return Err(error_for_status(url, response).await);
        }

        Ok(WaterfallsSubscription {
            response,
            parser: WaterfallsSseParser::default(),
            pending: VecDeque::new(),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) async fn get_history_waterfalls<S: WolletState>(
        &mut self,
        descriptor: &WolletDescriptor,
        cache: &S,
        to_index: u32,
    ) -> Result<Data, Error> {
        self.inner
            .get_history_waterfalls(descriptor, cache, to_index)
            .await
    }

    /// Return the descriptor string to use with Waterfalls descriptor endpoints.
    ///
    /// The returned descriptor has key origin information stripped and is encrypted
    /// for the Waterfalls server recipient unless descriptor encryption has been
    /// explicitly disabled on this client.
    pub async fn waterfalls_descriptor(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<String, Error> {
        #[allow(deprecated)]
        self.inner.waterfalls_descriptor(descriptor).await
    }

    /// Return the Waterfalls descriptor subscription URL for browser EventSource clients.
    ///
    /// The URL uses the same descriptor preparation as [`Self::waterfalls_descriptor`]:
    /// key origin information is stripped and the descriptor is encrypted unless
    /// descriptor encryption has been explicitly disabled on this client.
    pub async fn waterfalls_subscribe_url(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<String, Error> {
        #[allow(deprecated)]
        let desc = self.inner.waterfalls_descriptor(descriptor).await?;
        Ok(waterfalls_subscribe_url(&self.inner.base_url, &desc))
    }

    /// Subscribe to Waterfalls descriptor updates.
    ///
    /// Subscription events are hints. Callers remain responsible for running a
    /// normal scan after wallet invalidation events and for reopening the stream
    /// after reconnects or scans that expand the watched derivation range.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn subscribe(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<WaterfallsSubscription, Error> {
        let url = self.waterfalls_subscribe_url(descriptor).await?;
        self.subscribe_url(&url).await
    }

    /// Subscribe to Waterfalls descriptor updates and reopen the stream after disconnects.
    ///
    /// This preserves the normal subscription event stream and adds local
    /// `Disconnected` and `Reconnected` lifecycle updates. A reconnect is an
    /// invalidation hint: callers should run a normal scan because server events
    /// may have been missed while the stream was disconnected.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn subscribe_reconnecting(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<WaterfallsReconnectingSubscription, Error> {
        let url = self.waterfalls_subscribe_url(descriptor).await?;
        let subscription = self.subscribe_url(&url).await?;
        Ok(WaterfallsReconnectingSubscription {
            client: self.clone(),
            url,
            subscription: Some(subscription),
        })
    }

    /// Query the last used derivation index for a descriptor from the Waterfalls server.
    pub async fn last_used_index(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<LastUsedIndexResponse, Error> {
        self.inner.last_used_index(descriptor).await
    }

    /// Avoid encrypting the descriptor when calling the Waterfalls endpoint.
    pub fn avoid_encryption(&mut self) {
        #[allow(deprecated)]
        self.inner.avoid_encryption();
    }

    /// Returns the Waterfalls server recipient key using a cached value or by asking the server its key.
    pub async fn waterfalls_server_recipient(&mut self) -> Result<Recipient, Error> {
        self.inner.waterfalls_server_recipient().await
    }

    /// Set the Waterfalls server recipient key.
    ///
    /// This is used to encrypt the descriptor when calling the Waterfalls endpoint.
    pub fn set_waterfalls_server_recipient(&mut self, recipient: Recipient) {
        #[allow(deprecated)]
        self.inner.set_waterfalls_server_recipient(recipient);
    }

    /// Return the number of network requests made by this client.
    pub fn requests(&self) -> usize {
        self.inner.requests()
    }

    /// Whether the client is configured to only fetch transactions with unspent outputs.
    pub fn utxo_only(&self) -> bool {
        self.inner.utxo_only
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn capabilities(&self) -> HashSet<Capability> {
        self.inner.capabilities()
    }

    /// Returns true if the wallet has any tx using the first gap limit addresses.
    pub async fn has_txs(
        &self,
        descriptor: &WolletDescriptor,
        gap_limit: Option<u32>,
    ) -> Result<bool, Error> {
        self.inner.has_txs(descriptor, gap_limit).await
    }
}

impl WaterfallsClientBuilder {
    /// Consume the builder and build a new [`WaterfallsClient`].
    pub fn build(self) -> Result<WaterfallsClient, Error> {
        let mut builder = self.inner;
        builder.waterfalls = true;
        Ok(WaterfallsClient {
            inner: builder.build()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::Duration,
    };

    use crate::{
        clients::{
            asyncr::WaterfallsClientBuilder, WaterfallsSubscriptionEventKind,
            WaterfallsSubscriptionUpdate,
        },
        Error, Network, WolletDescriptor,
    };

    fn descriptor() -> WolletDescriptor {
        lwk_test_util::TEST_DESCRIPTOR.parse().unwrap()
    }

    fn serve_sequential_sse_responses(
        responses: Vec<(&'static str, bool)>,
    ) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let thread_accepted = Arc::clone(&accepted);

        thread::spawn(move || {
            for (body, keep_open) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                thread_accepted.fetch_add(1, Ordering::Relaxed);

                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{body}"
                );
                let _ = stream.write_all(response.as_bytes());

                if keep_open {
                    thread::sleep(Duration::from_secs(30));
                }
            }
        });

        (format!("http://{addr}"), accepted)
    }

    #[tokio::test]
    async fn subscribe_next_update_ignores_ready_and_returns_events() {
        let base_url = lwk_test_util::serve_http_response(
            "200 OK",
            "text/event-stream",
            ": ready\n\nevent: update\ndata: {\"type\":\"tip\"}\n\nevent: update\ndata: {\"type\":\"mempool\"}\n\n",
            false,
        );
        let mut client = WaterfallsClientBuilder::new(&base_url, Network::Liquid)
            .build()
            .unwrap();
        client.avoid_encryption();

        let mut subscription = client.subscribe(&descriptor()).await.unwrap();

        let first = subscription.next_update().await.unwrap().unwrap();
        assert_eq!(first.kind, WaterfallsSubscriptionEventKind::Tip);

        let second = subscription.next_update().await.unwrap().unwrap();
        assert_eq!(second.kind, WaterfallsSubscriptionEventKind::Mempool);

        assert!(subscription.next_update().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn subscribe_reconnecting_reopens_after_eof() {
        let (base_url, accepted) = serve_sequential_sse_responses(vec![
            (
                ": ready\n\nevent: update\ndata: {\"type\":\"tip\"}\n\n",
                false,
            ),
            (
                ": ready\n\nevent: update\ndata: {\"type\":\"mempool\"}\n\n",
                true,
            ),
        ]);
        let mut client = WaterfallsClientBuilder::new(&base_url, Network::Liquid)
            .build()
            .unwrap();
        client.avoid_encryption();

        let mut subscription = client.subscribe_reconnecting(&descriptor()).await.unwrap();

        match subscription.next_update().await.unwrap() {
            WaterfallsSubscriptionUpdate::Event(event) => {
                assert_eq!(event.kind, WaterfallsSubscriptionEventKind::Tip);
            }
            other => panic!("expected tip event, got {other:?}"),
        }

        assert_eq!(
            subscription.next_update().await.unwrap(),
            WaterfallsSubscriptionUpdate::Disconnected { error: None }
        );
        assert_eq!(
            subscription.next_update().await.unwrap(),
            WaterfallsSubscriptionUpdate::Reconnected
        );

        match subscription.next_update().await.unwrap() {
            WaterfallsSubscriptionUpdate::Event(event) => {
                assert_eq!(event.kind, WaterfallsSubscriptionEventKind::Mempool);
            }
            other => panic!("expected mempool event, got {other:?}"),
        }

        assert_eq!(accepted.load(Ordering::Relaxed), 2);
    }

    /// Run with:
    ///
    /// `RUST_LOG=info direnv exec . cargo test -p lwk_wollet --lib manual_subscribe_reconnecting_production_testnet -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "manual hanging test for toggling network connectivity"]
    async fn manual_subscribe_reconnecting_production_testnet() {
        lwk_test_util::init_logging();

        let base_url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
        let mut client = WaterfallsClientBuilder::new(base_url, Network::TestnetLiquid)
            .build()
            .unwrap();
        let mut subscription = client.subscribe_reconnecting(&descriptor()).await.unwrap();

        log::info!("subscribed to {base_url}; toggle Wi-Fi off and on, or press Ctrl-C to stop");
        loop {
            match subscription.next_update().await {
                Ok(update) => log::info!("subscription update: {update:?}"),
                Err(err) => {
                    log::info!("reconnect attempt failed: {err}; retrying in one second");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    #[tokio::test]
    async fn subscribe_returns_error_for_422() {
        let base_url = lwk_test_util::serve_http_response(
            "422 Unprocessable Entity",
            "text/event-stream",
            "CannotDecrypt",
            false,
        );
        let mut client = WaterfallsClientBuilder::new(&base_url, Network::Liquid)
            .build()
            .unwrap();
        client.avoid_encryption();

        let err = match client.subscribe(&descriptor()).await {
            Ok(_) => panic!("subscribe should fail"),
            Err(err) => err,
        };
        assert!(matches!(err, Error::EsploraHttpError { status: 422, .. }));
    }
}
