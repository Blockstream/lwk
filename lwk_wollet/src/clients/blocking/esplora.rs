use age::x25519::Recipient;
use elements::{BlockHash, Script, Txid};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;

use crate::{
    async_util::with_timeout,
    cache::Height,
    clients::{asyncr, Capability, Data, EsploraClientBuilder, History},
    update::Update,
    wollet::WolletState,
    Error, Network, WolletDescriptor,
};

use super::{full_scan_to_index_default, BlockchainBackend};

impl EsploraClientBuilder {
    /// Build a blocking Esplora client
    pub fn build_blocking(self) -> Result<EsploraClient, Error> {
        Ok(EsploraClient {
            rt: Runtime::new()?,
            scan_deadline: None,
            client: EsploraClientBuilder::build(self)?,
        })
    }
}

#[derive(Debug)]
/// A blockchain backend implementation based on the
/// [esplora HTTP API](https://github.com/blockstream/esplora/blob/master/API.md)
/// But can also use the [waterfalls](https://github.com/RCasatta/waterfalls) endpoint to speed up the scan if supported by the server.
pub struct EsploraClient {
    rt: Runtime,
    client: asyncr::EsploraClient,

    /// Deadline of the running full scan, `None` when no full scan is running.
    ///
    /// A deadline and not a timeout, so the whole scan is bounded, see [`ScanDeadline`].
    scan_deadline: Option<ScanDeadline>,
}

/// The instant by which a blocking full scan must complete.
///
/// A blocking full scan is not a single future: the trait's default `full_scan_to_index` makes
/// several separate `block_on` calls (history, tip, transactions, headers), each with its own
/// network requests. Bounding every call with the scan timeout would let a scan last several
/// times the timeout: 10 minutes for the history, then 10 more for the transactions, and so on.
///
/// Instead the deadline is fixed when the scan starts, and every call is bounded by the time
/// left until it, so the whole scan completes within the scan timeout.
/// See [`asyncr::EsploraClient::scan_timeout()`].
#[derive(Debug, Clone, Copy)]
pub(super) struct ScanDeadline {
    at: Instant,
    timeout: Duration,
}

impl ScanDeadline {
    /// Start a deadline of `timeout` from now, `None` if it is too far to be represented.
    pub(super) fn start(timeout: Duration) -> Option<Self> {
        let at = Instant::now().checked_add(timeout)?;
        Some(Self { at, timeout })
    }
}

/// Run `future` on `rt`, bounded by the remaining time of `deadline` if any.
pub(super) fn block_on_scan<T>(
    rt: &Runtime,
    deadline: Option<ScanDeadline>,
    future: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    match deadline {
        Some(deadline) => {
            let remaining = deadline.at.saturating_duration_since(Instant::now());
            rt.block_on(with_timeout(remaining, future, || {
                Error::ScanTimeout(deadline.timeout)
            }))
        }
        None => rt.block_on(future),
    }
}

impl EsploraClient {
    /// Create a new Esplora client
    pub fn new(url: &str, network: Network) -> Result<Self, Error> {
        Ok(Self {
            rt: Runtime::new()?,
            scan_deadline: None,
            client: asyncr::EsploraClient::new(network, url),
        })
    }
}

/// "Waterfalls" methods
impl EsploraClient {
    /// Create a new Esplora client using the "waterfalls" endpoint
    ///
    /// Prefer [`crate::clients::blocking::WaterfallsClient`] for new Waterfalls clients.
    #[deprecated(since = "0.18.2", note = "use WaterfallsClient::new instead")]
    pub fn new_waterfalls(url: &str, network: Network) -> Result<Self, Error> {
        Ok(Self {
            rt: Runtime::new()?,
            scan_deadline: None,
            #[allow(deprecated)]
            client: EsploraClientBuilder::new(url, network)
                .waterfalls(true)
                .build()?,
        })
    }

    /// Do not encrypt the descriptor when using the "waterfalls" endpoint
    #[deprecated(
        since = "0.18.2",
        note = "use WaterfallsClient::avoid_encryption instead"
    )]
    pub fn waterfalls_avoid_encryption(&mut self) {
        self.client.waterfalls_avoid_encryption = true;
    }

    /// Returns the waterfall server recipient key using a cached value or by asking the server its key
    #[deprecated(
        since = "0.18.2",
        note = "use WaterfallsClient::waterfalls_server_recipient instead"
    )]
    pub fn waterfalls_server_recipient(&mut self) -> Result<Recipient, Error> {
        self.rt.block_on(self.client.waterfalls_server_recipient())
    }

    /// Set the waterfalls server recipient key. This is used to encrypt the descriptor when calling the waterfalls endpoint.
    #[deprecated(
        since = "0.18.2",
        note = "use WaterfallsClient::set_waterfalls_server_recipient instead"
    )]
    pub fn set_waterfalls_server_recipient(&mut self, recipient: Recipient) {
        #[allow(deprecated)]
        self.client.set_waterfalls_server_recipient(recipient);
    }

    /// Return the descriptor string to use with Waterfalls descriptor endpoints.
    ///
    /// This compatibility primitive is used internally by
    /// [`crate::clients::blocking::WaterfallsClient`]
    /// for Waterfalls scans and subscriptions.
    #[deprecated(
        since = "0.18.2",
        note = "use WaterfallsClient::waterfalls_descriptor instead"
    )]
    pub fn waterfalls_descriptor(
        &mut self,
        descriptor: &WolletDescriptor,
    ) -> Result<String, Error> {
        #[allow(deprecated)]
        self.rt
            .block_on(self.client.waterfalls_descriptor(descriptor))
    }
}

impl BlockchainBackend for EsploraClient {
    fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        block_on_scan(&self.rt, self.scan_deadline, self.client.tip())
    }

    fn broadcast(&self, tx: &elements::Transaction) -> Result<elements::Txid, crate::Error> {
        self.rt.block_on(self.client.broadcast(tx))
    }

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<elements::Transaction>, Error> {
        block_on_scan(
            &self.rt,
            self.scan_deadline,
            self.client.get_transactions(txids),
        )
    }

    fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        block_on_scan(
            &self.rt,
            self.scan_deadline,
            self.client.get_headers(heights, height_blockhash),
        )
    }

    // examples:
    // https://blockstream.info/liquidtestnet/api/address/tex1qntw9m0j2e93n84x975t47ddhgkzx3x8lhfv2nj/txs
    // https://blockstream.info/liquidtestnet/api/scripthash/b50a2a798d876db54acfa0d8dfdc49154ea8defed37b225ec4c9ec7415358ba3/txs
    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        block_on_scan(
            &self.rt,
            self.scan_deadline,
            self.client.get_scripts_history(scripts),
        )
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
        block_on_scan(
            &self.rt,
            self.scan_deadline,
            self.client
                .get_history_waterfalls(descriptor, state, to_index),
        )
    }

    fn utxo_only(&self) -> bool {
        self.client.utxo_only
    }

    fn full_scan_to_index<S: WolletState>(
        &mut self,
        state: &S,
        index: u32,
    ) -> Result<Option<Update>, Error> {
        self.scan_deadline = ScanDeadline::start(self.client.scan_timeout());
        let result = full_scan_to_index_default(self, state, index);
        self.scan_deadline = None;
        result
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::Duration};

    use super::EsploraClient;
    use crate::{
        clients::{blocking::BlockchainBackend, EsploraClientBuilder},
        Error, Network, WolletBuilder,
    };
    use elements::{encode::Decodable, BlockHash};

    fn get_block(base_url: &str, hash: BlockHash) -> elements::Block {
        let url = format!("{base_url}/block/{hash}/raw");
        let response = reqwest::blocking::get(url).unwrap();
        elements::Block::consensus_decode(&response.bytes().unwrap()[..]).unwrap()
    }

    #[ignore = "Should be integration test, but it is testing private function"]
    #[test]
    fn esplora_local() {
        let env = lwk_test_util::TestEnvBuilder::from_env()
            .with_esplora()
            .build();

        test_esplora_url(&env.esplora_url());
    }

    #[ignore]
    #[test]
    fn esplora_testnet() {
        test_esplora_url("https://blockstream.info/liquidtestnet/api");
        test_esplora_url("https://liquid.network/liquidtestnet/api");
    }

    fn test_esplora_url(esplora_url: &str) {
        println!("{esplora_url}");

        let mut client = EsploraClient::new(esplora_url, Network::default_regtest()).unwrap();
        let header = client.tip().unwrap();
        assert!(header.height > 100);

        let headers = client.get_headers(&[0], &HashMap::new()).unwrap();
        let genesis_header = &headers[0];
        assert_eq!(genesis_header.height, 0);

        let genesis_block = get_block(esplora_url, genesis_header.block_hash());
        let genesis_tx = &genesis_block.txdata[0];

        let txid = genesis_tx.txid();
        let txs = client.get_transactions(&[txid]).unwrap();

        assert_eq!(txs[0].txid(), txid);

        // Test get_transaction method
        let _tx = txs[0].clone();

        // Test get_transactions method with the same txid
        let txs_batch = client.get_transactions(&[txid]).unwrap();
        assert_eq!(txs_batch.len(), 1);
        assert_eq!(txs_batch[0].txid(), txid);

        // Test get_transactions with multiple txids if there are more transactions in the block
        if genesis_block.txdata.len() > 1 {
            let txid2 = genesis_block.txdata[1].txid();
            let txids = vec![txid, txid2];
            let txs_multi = client.get_transactions(&txids).unwrap();
            assert_eq!(txs_multi.len(), 2);
            assert_eq!(txs_multi[0].txid(), txid);
            assert_eq!(txs_multi[1].txid(), txid2);
        }
    }

    #[test]
    fn esplora_full_scan_is_bounded() {
        let wollet = WolletBuilder::new(
            Network::Liquid,
            lwk_test_util::TEST_DESCRIPTOR.parse().unwrap(),
        )
        .build()
        .unwrap();

        // The connection is kept open, so the body never completes
        let keep_connection_open = true;
        let base_url = lwk_test_util::serve_http_response(
            "200 OK",
            "application/json",
            "[]",
            keep_connection_open,
        );
        // Shorter than the per-request timeout, so only the scan deadline can fire
        let scan_timeout = Duration::from_millis(100);
        let builder =
            EsploraClientBuilder::new(&base_url, Network::Liquid).scan_timeout(scan_timeout);

        let mut client = builder.clone().build_blocking().unwrap();
        let err = client.full_scan(&wollet).unwrap_err();
        assert!(matches!(err, Error::ScanTimeout(t) if t == scan_timeout));

        let mut client = builder.build().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let err = rt.block_on(client.full_scan(&wollet)).unwrap_err();
        assert!(matches!(err, Error::ScanTimeout(t) if t == scan_timeout));

        let keep_connection_open = false;
        let base_url = lwk_test_util::serve_http_response(
            "200 OK",
            "application/json",
            "[]",
            keep_connection_open,
        );
        let mut client = EsploraClientBuilder::new(&base_url, Network::Liquid)
            .max_response_size(1)
            .build_blocking()
            .unwrap();
        let err = client.full_scan(&wollet).unwrap_err();
        assert!(matches!(err, Error::ResponseTooLarge { limit: 1 }));
    }
}
