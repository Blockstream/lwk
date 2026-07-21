use std::{
    collections::{HashMap, HashSet},
    sync::{mpsc, Mutex},
    thread,
    time::Duration,
};

use age::x25519::Recipient;
use elements::{BlockHash, Script, Txid};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

use crate::{
    cache::Height,
    clients::{
        asyncr, Capability, Data, History, WaterfallsClientBuilder, WaterfallsSubscriptionEvent,
        WaterfallsSubscriptionUpdate,
    },
    wollet::WolletState,
    Error, Network, WolletDescriptor,
};

use super::BlockchainBackend;

impl WaterfallsClientBuilder {
    /// Build a blocking Waterfalls client.
    pub fn build_blocking(self) -> Result<WaterfallsClient, Error> {
        Ok(WaterfallsClient {
            rt: Runtime::new()?,
            client: WaterfallsClientBuilder::build(self)?,
        })
    }
}

#[derive(Debug)]
/// A blockchain backend implementation based on the
/// [Waterfalls HTTP API](https://github.com/RCasatta/waterfalls).
///
/// Waterfalls is Esplora-compatible for common chain operations and adds
/// descriptor-based wallet scan endpoints.
pub struct WaterfallsClient {
    rt: Runtime,
    client: asyncr::WaterfallsClient,
}

/// A blocking Waterfalls descriptor subscription stream.
///
/// The stream owns a worker thread that reads the async SSE stream. Use
/// [`Self::close`] to stop the worker and interrupt a blocked [`Self::next_update`].
pub struct WaterfallsSubscription {
    updates: Mutex<mpsc::Receiver<Result<Option<WaterfallsSubscriptionEvent>, Error>>>,
    close: Mutex<Option<oneshot::Sender<()>>>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
}

/// A blocking Waterfalls descriptor subscription stream that can reopen itself.
///
/// The stream owns a worker thread that reads the async SSE stream. Use
/// [`Self::close`] to stop the worker and interrupt a blocked [`Self::next_update`].
pub struct WaterfallsReconnectingSubscription {
    updates: Mutex<mpsc::Receiver<Result<Option<WaterfallsSubscriptionUpdate>, Error>>>,
    close: Mutex<Option<oneshot::Sender<()>>>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
}

impl WaterfallsSubscription {
    fn new(subscription: asyncr::WaterfallsSubscription) -> Result<Self, Error> {
        let (updates_tx, updates_rx) = mpsc::channel();
        let (close_tx, mut close_rx) = oneshot::channel();
        let rt = Runtime::new()?;
        let worker = thread::spawn(move || {
            rt.block_on(async move {
                let mut subscription = subscription;
                loop {
                    tokio::select! {
                        _ = &mut close_rx => break,
                        event = subscription.next_update() => {
                            match event {
                                Ok(Some(event)) => {
                                    if updates_tx.send(Ok(Some(event))).is_err() {
                                        break;
                                    }
                                }
                                Ok(None) => {
                                    let _ = updates_tx.send(Ok(None));
                                    break;
                                }
                                Err(err) => {
                                    let _ = updates_tx.send(Err(err));
                                    break;
                                }
                            }
                        }
                    }
                }
            });
        });

        Ok(Self {
            updates: Mutex::new(updates_rx),
            close: Mutex::new(Some(close_tx)),
            worker: Mutex::new(Some(worker)),
        })
    }

    /// Return the next Waterfalls subscription update.
    ///
    /// Returns `Ok(None)` when the server closes the stream or [`Self::close`]
    /// is called.
    pub fn next_update(&self) -> Result<Option<WaterfallsSubscriptionEvent>, Error> {
        match self
            .updates
            .lock()
            .map_err(|_| Error::Generic("subscription receiver mutex poisoned".to_string()))?
            .recv()
        {
            Ok(event) => event,
            Err(_) => Ok(None),
        }
    }

    /// Stop the subscription stream and release its worker thread.
    pub fn close(&self) {
        if let Ok(mut close) = self.close.lock() {
            if let Some(close) = close.take() {
                let _ = close.send(());
            }
        }

        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
    }
}

impl Drop for WaterfallsSubscription {
    fn drop(&mut self) {
        self.close();
    }
}

impl WaterfallsReconnectingSubscription {
    fn new(subscription: asyncr::WaterfallsReconnectingSubscription) -> Result<Self, Error> {
        let (updates_tx, updates_rx) = mpsc::channel();
        let (close_tx, mut close_rx) = oneshot::channel();
        let rt = Runtime::new()?;
        let worker = thread::spawn(move || {
            rt.block_on(async move {
                let mut subscription = subscription;
                loop {
                    tokio::select! {
                        _ = &mut close_rx => break,
                        update = subscription.next_update() => {
                            match update {
                                Ok(update) => {
                                    if updates_tx.send(Ok(Some(update))).is_err() {
                                        break;
                                    }
                                }
                                Err(err) => {
                                    if updates_tx.send(Err(err)).is_err() {
                                        break;
                                    }
                                    tokio::select! {
                                        _ = &mut close_rx => break,
                                        _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                                    }
                                }
                            }
                        }
                    }
                }
            });
        });

        Ok(Self {
            updates: Mutex::new(updates_rx),
            close: Mutex::new(Some(close_tx)),
            worker: Mutex::new(Some(worker)),
        })
    }

    /// Return the next reconnecting Waterfalls subscription update.
    ///
    /// Returns `Ok(None)` when [`Self::close`] is called.
    pub fn next_update(&self) -> Result<Option<WaterfallsSubscriptionUpdate>, Error> {
        match self
            .updates
            .lock()
            .map_err(|_| Error::Generic("subscription receiver mutex poisoned".to_string()))?
            .recv()
        {
            Ok(update) => update,
            Err(_) => Ok(None),
        }
    }

    /// Stop the subscription stream and release its worker thread.
    pub fn close(&self) {
        if let Ok(mut close) = self.close.lock() {
            if let Some(close) = close.take() {
                let _ = close.send(());
            }
        }

        if let Ok(mut worker) = self.worker.lock() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
    }
}

impl Drop for WaterfallsReconnectingSubscription {
    fn drop(&mut self) {
        self.close();
    }
}

impl WaterfallsClient {
    /// Create a new Waterfalls client.
    pub fn new(url: &str, network: Network) -> Result<Self, Error> {
        Ok(Self {
            rt: Runtime::new()?,
            client: asyncr::WaterfallsClient::new(network, url),
        })
    }

    /// Do not encrypt the descriptor when using the Waterfalls endpoint.
    pub fn avoid_encryption(&mut self) {
        self.client.avoid_encryption();
    }

    /// Returns the Waterfalls server recipient key using a cached value or by asking the server its key.
    pub fn waterfalls_server_recipient(&mut self) -> Result<Recipient, Error> {
        self.rt.block_on(self.client.waterfalls_server_recipient())
    }

    /// Set the Waterfalls server recipient key.
    ///
    /// This is used to encrypt the descriptor when calling the Waterfalls endpoint.
    pub fn set_waterfalls_server_recipient(&mut self, recipient: Recipient) {
        self.client.set_waterfalls_server_recipient(recipient);
    }

    /// Return the descriptor string to use with Waterfalls descriptor endpoints.
    pub fn waterfalls_descriptor(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<String, Error> {
        self.rt
            .block_on(self.client.waterfalls_descriptor(descriptor))
    }

    /// Query the last used derivation index for a descriptor from the Waterfalls server.
    pub fn last_used_index(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<asyncr::LastUsedIndexResponse, Error> {
        self.rt.block_on(self.client.last_used_index(descriptor))
    }

    /// Subscribe to Waterfalls descriptor updates.
    ///
    /// Subscription events are hints. Callers remain responsible for running a
    /// normal scan after wallet invalidation events and for reopening the stream
    /// after reconnects or scans that expand the watched derivation range.
    pub fn subscribe(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<WaterfallsSubscription, Error> {
        let subscription = self.rt.block_on(self.client.subscribe(descriptor))?;
        WaterfallsSubscription::new(subscription)
    }

    /// Subscribe to Waterfalls descriptor updates and reopen the stream after disconnects.
    pub fn subscribe_reconnecting(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<WaterfallsReconnectingSubscription, Error> {
        let subscription = self
            .rt
            .block_on(self.client.subscribe_reconnecting(descriptor))?;
        WaterfallsReconnectingSubscription::new(subscription)
    }
}

impl BlockchainBackend for WaterfallsClient {
    fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        self.rt.block_on(self.client.tip())
    }

    fn broadcast(&self, tx: &elements::Transaction) -> Result<elements::Txid, crate::Error> {
        self.rt.block_on(self.client.broadcast(tx))
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<elements::Transaction>, Error> {
        self.rt.block_on(self.client.get_transactions(txids))
    }

    fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        self.rt
            .block_on(self.client.get_headers(heights, height_blockhash))
    }

    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        self.rt.block_on(self.client.get_scripts_history(scripts))
    }

    fn capabilities(&self) -> HashSet<Capability> {
        self.client.capabilities()
    }

    fn get_history_waterfalls<S: WolletState>(
        &mut self,
        descriptor: &WolletDescriptor,
        state: &S,
        to_index: u32,
    ) -> Result<Data, Error> {
        self.rt.block_on(
            self.client
                .get_history_waterfalls(descriptor, state, to_index),
        )
    }

    fn utxo_only(&self) -> bool {
        self.client.utxo_only()
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
    };

    use crate::{
        clients::{
            blocking::WaterfallsClient, WaterfallsSubscriptionEventKind,
            WaterfallsSubscriptionUpdate,
        },
        Network, WolletDescriptor,
    };

    fn descriptor() -> WolletDescriptor {
        lwk_test_util::TEST_DESCRIPTOR.parse().unwrap()
    }

    fn serve_sequential_responses(responses: Vec<&'static str>) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let thread_accepted = Arc::clone(&accepted);

        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                thread_accepted.fetch_add(1, Ordering::Relaxed);

                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(response.as_bytes());
            }
        });

        (format!("http://{addr}"), accepted)
    }

    #[test]
    fn blocking_subscribe_next_update_returns_events_and_eof() {
        let base_url = lwk_test_util::serve_http_response(
            "200 OK",
            "text/event-stream",
            ": ready\n\nevent: update\ndata: {\"type\":\"tip\"}\n\nevent: update\ndata: {\"type\":\"block\"}\n\n",
            false,
        );
        let mut client = WaterfallsClient::new(&base_url, Network::Liquid).unwrap();
        client.avoid_encryption();

        let subscription = client.subscribe(&descriptor()).unwrap();

        let first = subscription.next_update().unwrap().unwrap();
        assert_eq!(first.kind, WaterfallsSubscriptionEventKind::Tip);

        let second = subscription.next_update().unwrap().unwrap();
        assert_eq!(second.kind, WaterfallsSubscriptionEventKind::Block);

        assert!(subscription.next_update().unwrap().is_none());
    }

    #[test]
    fn blocking_subscribe_close_interrupts_next_update() {
        let base_url =
            lwk_test_util::serve_http_response("200 OK", "text/event-stream", ": ready\n\n", true);
        let mut client = WaterfallsClient::new(&base_url, Network::Liquid).unwrap();
        client.avoid_encryption();

        let subscription = Arc::new(client.subscribe(&descriptor()).unwrap());
        let worker_subscription = Arc::clone(&subscription);
        let worker = thread::spawn(move || worker_subscription.next_update().unwrap());

        subscription.close();

        assert!(worker.join().unwrap().is_none());
    }

    #[test]
    fn blocking_subscribe_reconnecting_recovers_after_error() {
        let (base_url, accepted) = serve_sequential_responses(vec![
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n: ready\n\n",
            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n: ready\n\n",
        ]);
        let mut client = WaterfallsClient::new(&base_url, Network::Liquid).unwrap();
        client.avoid_encryption();

        let subscription = client.subscribe_reconnecting(&descriptor()).unwrap();

        assert_eq!(
            subscription.next_update().unwrap(),
            Some(WaterfallsSubscriptionUpdate::Disconnected { error: None })
        );
        assert!(subscription.next_update().is_err());
        assert_eq!(
            subscription.next_update().unwrap(),
            Some(WaterfallsSubscriptionUpdate::Reconnected)
        );
        assert_eq!(accepted.load(Ordering::Relaxed), 3);
    }
}
