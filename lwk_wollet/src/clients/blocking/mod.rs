//! Blocking clients to fetch data from the Blockchain.

use crate::{
    cache::{Height, Timestamp, BATCH_SIZE},
    clients::try_unblind,
    update::{DownloadTxResult, Update},
    wollet::WolletState,
    BlindingPublicKey, Chain, Error, WolletDescriptor,
};
use elements::{bitcoin::bip32::ChildNumber, OutPoint, Script};
use elements::{BlockHash, BlockHeader, Transaction, Txid};
use std::collections::{HashMap, HashSet};

#[cfg(feature = "esplora")]
mod esplora;
#[cfg(feature = "esplora")]
mod waterfalls;

#[cfg(feature = "esplora")]
pub use esplora::EsploraClient;
#[cfg(feature = "esplora")]
pub use waterfalls::{
    WaterfallsClient, WaterfallsReconnectingSubscription, WaterfallsSubscription,
};

#[cfg(feature = "elements_rpc")]
pub use elements_rpc_client::ElementsRpcClient;

use super::{Capability, Data, History, LastUnused};

#[cfg(feature = "electrum")]
pub(crate) mod electrum_client;

#[cfg(feature = "elements_rpc")]
pub(crate) mod elements_rpc_client;

/// Trait implemented by types that can fetch data from a blockchain data source.
pub trait BlockchainBackend {
    /// Get the blockchain latest block header
    fn tip(&mut self) -> Result<BlockHeader, Error>;

    /// Broadcast a transaction
    fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error>;

    /// Get a list of transactions
    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<Transaction>, Error>;

    /// Get a list of block headers
    ///
    /// Optionally pass the blockhash if already known
    fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<BlockHeader>, Error>;

    /// Get the transactions involved in a list of scripts
    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error>;

    /// Return the set of [`Capability`] supported by this backend
    fn capabilities(&self) -> HashSet<Capability> {
        HashSet::new()
    }

    /// The silent payment tweak `T = input_hash·A` of every eligible transaction in
    /// the block at `height`, paired with its txid.
    ///
    /// Returns each eligible transaction's `T = input_hash·A` tweak in a block.
    ///
    /// Defaults to [`Error::SilentPaymentsUnsupportedByBackend`].
    #[cfg(feature = "silentpayments")]
    fn silent_payment_tweaks(
        &self,
        _height: Height,
    ) -> Result<Vec<(Txid, crate::silentpayments::PartialTweak)>, Error> {
        Err(Error::SilentPaymentsUnsupportedByBackend)
    }

    /// Whether the client is configured to only fetch transactions with unspent outputs (false by default)
    fn utxo_only(&self) -> bool {
        false
    }

    /// Get the wallet history
    fn get_history<S: WolletState>(
        &mut self,
        descriptor: &WolletDescriptor,
        state: &S,
        index: u32,
        last_unused: LastUnused,
    ) -> Result<Data, Error> {
        let mut data = Data::default();

        for descriptor in descriptor.as_single_descriptors()? {
            let mut batch_count = 0;
            let chain: Chain = (&descriptor).try_into().unwrap_or(Chain::External);
            let index = index.max(last_unused[chain]);
            loop {
                let batch = state.get_script_batch(batch_count, chain)?;

                let s: Vec<_> = batch.value.iter().map(|e| &e.0).collect();
                let result: Vec<Vec<History>> = self.get_scripts_history(&s)?;
                if !batch.cached {
                    data.scripts.extend(batch.value);
                }
                let max = result
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| !v.is_empty())
                    .map(|(i, _)| i as u32)
                    .max();
                if let Some(max) = max {
                    match chain {
                        Chain::External => {
                            data.last_unused.external = 1 + max + batch_count * BATCH_SIZE
                        }
                        Chain::Internal => {
                            data.last_unused.internal = 1 + max + batch_count * BATCH_SIZE
                        }
                    }
                };

                let flattened: Vec<History> = result.into_iter().flatten().collect();

                if flattened.is_empty() && index <= 1 + batch_count * BATCH_SIZE {
                    break;
                }

                for el in flattened {
                    // el.height = -1 means unconfirmed with unconfirmed parents
                    // el.height =  0 means unconfirmed with confirmed parents
                    // but we threat those tx the same
                    let height = el.height.max(0);
                    let txid = el.txid;
                    if height == 0 {
                        data.txid_height.insert(txid, None);
                    } else {
                        data.txid_height.insert(txid, Some(height as u32));
                        if let Some(block_hash) = el.block_hash {
                            data.height_blockhash.insert(height as u32, block_hash);
                        }
                    }
                }

                batch_count += 1;

                if !descriptor.has_wildcard() {
                    // No wildcard, 1 loop is enough
                    return Ok(data);
                }
            }
        }
        Ok(data)
    }

    /// Get the history using the waterfalls endpoint
    fn get_history_waterfalls<S: WolletState>(
        &mut self,
        _descriptor: &WolletDescriptor,
        _state: &S,
        _to_index: u32,
    ) -> Result<Data, Error> {
        Err(Error::WaterfallsUnimplemented)
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
    /// To scan beyond the gap limit use [`BlockchainBackend::full_scan_to_index()`] instead.
    fn full_scan<S: WolletState>(&mut self, state: &S) -> Result<Option<Update>, Error> {
        self.full_scan_to_index(state, 0)
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet up to a specified derivation index
    ///
    /// While [`BlockchainBackend::full_scan()`] stops after finding 20 consecutive unused addresses (the gap limit),
    /// this method will scan at least up to the given derivation index. This is useful to prevent
    /// missing funds in cases where outputs exist beyond the gap limit.
    ///
    /// Will scan both external and internal address chains up to the given index for maximum safety,
    /// even though internal addresses may not need such deep scanning.
    ///
    /// If transactions are found beyond the gap limit during this scan, subsequent calls to
    /// [`BlockchainBackend::full_scan()`] will automatically scan up to the highest used index, preventing any
    /// previously-found transactions from being missed.
    ///
    /// See [`crate::asyncr::EsploraClient::full_scan_to_index()`] for an async version of this method.
    fn full_scan_to_index<S: WolletState>(
        &mut self,
        state: &S,
        index: u32,
    ) -> Result<Option<Update>, Error> {
        if state.utxo_only() != self.utxo_only() {
            return Err(Error::UtxoOnlyIncompatible);
        }
        let descriptor = state.descriptor();

        let Data {
            txid_height,
            scripts,
            last_unused,
            height_blockhash,
            height_timestamp: _height_timestamp,
            tip: _,
            unspent,
        } = if self.capabilities().contains(&Capability::Waterfalls) {
            match self.get_history_waterfalls(&descriptor, state, index) {
                Ok(d) => d,
                Err(Error::UsingWaterfallsWithElip151) => {
                    self.get_history(&descriptor, state, index, state.last_unused())?
                }
                Err(e) => return Err(e),
            }
        } else {
            self.get_history(&descriptor, state, index, state.last_unused())?
        };

        let tip = self.tip()?;

        let history_txs_id: HashSet<Txid> = txid_height.keys().cloned().collect();
        #[cfg_attr(not(feature = "silentpayments"), allow(unused_mut))]
        let mut new_txs = self.download_txs(&history_txs_id, &scripts, state, &descriptor)?;

        // Silent payments are discovered here rather than left to the caller because
        // they are wallet money like any other: a `full_scan` that skipped them would
        // report a balance that is missing funds the wallet can actually spend.
        #[cfg(feature = "silentpayments")]
        let silent_payments = self.scan_silent_payments_for(state, &tip, &mut new_txs)?;

        let history_txs_heights_plus_tip: HashSet<Height> = txid_height
            .values()
            .filter_map(|e| *e)
            .chain(std::iter::once(tip.height))
            .collect();
        let timestamps =
            self.download_headers(&history_txs_heights_plus_tip, &height_blockhash, state)?;

        let cache_last_unused_external = state.last_unused()[Chain::External];
        let cache_last_unused_internal = state.last_unused()[Chain::Internal];

        let last_unused_changed = cache_last_unused_external != last_unused.external
            || cache_last_unused_internal != last_unused.internal;

        let changed = !new_txs.txs.is_empty()
            || last_unused_changed
            || !scripts.is_empty()
            || !timestamps.is_empty()
            || state.tip() != (tip.height, tip.block_hash());

        if changed {
            log::debug!("something changed: !new_txs.txs.is_empty():{} last_unused_changed:{} !scripts.is_empty():{} !timestamps.is_empty():{}", !new_txs.txs.is_empty(), last_unused_changed, !scripts.is_empty(), !timestamps.is_empty() );

            let txid_height_new: Vec<_> = txid_height
                .iter()
                .filter(|(k, v)| match state.heights().get(*k) {
                    Some(e) => e != *v,
                    None => true,
                })
                .map(|(k, v)| (*k, *v))
                .collect();
            let txid_height_delete: Vec<_> = state
                .heights()
                .keys()
                .filter(|k| !txid_height.contains_key(*k))
                .cloned()
                .collect();
            let wollet_status = state.wollet_status();

            let scripts_with_blinding_pubkey: Vec<(_, _, _, _)> = scripts
                .iter()
                .map(|(script, (chain, child, blinding_pubkey))| {
                    (*chain, *child, script.clone(), *blinding_pubkey)
                })
                .collect();

            let update = Update {
                #[cfg(feature = "silentpayments")]
                version: if silent_payments.is_some() { 6 } else { 4 },
                #[cfg(not(feature = "silentpayments"))]
                version: 4,
                wollet_status,
                new_txs,
                txid_height_new,
                txid_height_delete,
                timestamps,
                scripts_with_blinding_pubkey,
                tip,
                unspent,
                last_unused,
                #[cfg(feature = "silentpayments")]
                silent_payments,
            };
            Ok(Some(update))
        } else {
            Ok(None)
        }
    }

    /// Run silent payment discovery as part of a full scan, appending any transactions
    /// it found to `new_txs` so the outputs have a transaction to hang off.
    ///
    /// Returns `None` when discovery did not run.
    #[cfg(feature = "silentpayments")]
    fn scan_silent_payments_for<S: WolletState>(
        &self,
        state: &S,
        tip: &BlockHeader,
        new_txs: &mut DownloadTxResult,
    ) -> Result<Option<crate::update::SilentPaymentsUpdate>, Error> {
        let Some(material) = state.silent_payment_material() else {
            return Ok(None);
        };
        if !self.capabilities().contains(&Capability::SilentPayments) {
            return Ok(None);
        }

        // Resume where discovery last reached, not from the wallet tip: the two advance
        // independently, and starting from the tip would skip every block between. On a
        // wallet that has never scanned this is the configured birthday.
        let from = state.silent_payments_scan_from();
        if from > tip.height {
            return Ok(None);
        }

        let sync = crate::silentpayments::SilentPaymentSync::new(*material)
            .with_labels([crate::silentpayments::CHANGE_LABEL]);
        let hits = self.scan_silent_payments_with_tweaks(&sync, from, tip.height)?;

        // Scan with the tweak discovery already computed. Deriving it again here would
        // need the prevout scriptPubKeys, which a light client does not have — passing
        // an empty prevout lookup would make every input ineligible and find nothing.
        let scanner = crate::silentpayments::SilentPaymentTxScanner::new(*material)
            .with_labels([crate::silentpayments::CHANGE_LABEL]);
        let mut found = Vec::new();
        for (tx, tweak) in &hits {
            for utxo in scanner.scan_tx_with_tweak(tx, tweak) {
                found.push((utxo.cache_entry(), utxo.unblinded));
            }
        }
        let txs: Vec<Transaction> = hits.into_iter().map(|(tx, _)| tx).collect();

        // The transactions discovery fetched are not in the descriptor-derived history,
        // so nothing else in this scan would add them. Without them the outputs would
        // have no confirmation height and `apply_silent_payments`-style bookkeeping
        // would have nothing to attach to.
        if !found.is_empty() {
            let known: HashSet<Txid> = new_txs.txs.iter().map(|(txid, _)| *txid).collect();
            for tx in txs {
                let txid = tx.txid();
                if !known.contains(&txid) {
                    new_txs.txs.push((txid, tx));
                }
            }
        }

        Ok(Some(crate::update::SilentPaymentsUpdate {
            found,
            scanned_to: tip.height,
        }))
    }

    /// Find the silent payments to `state`'s scan keys in the blocks `from_height..=to_height`.
    ///
    /// Returns the transactions that pay this wallet, for the caller to scan and apply.
    ///
    /// Requires [`Capability::SilentPayments`].
    #[cfg(feature = "silentpayments")]
    fn scan_silent_payments(
        &self,
        sync: &crate::silentpayments::SilentPaymentSync,
        from_height: Height,
        to_height: Height,
    ) -> Result<Vec<Transaction>, Error> {
        if !self.capabilities().contains(&Capability::SilentPayments) {
            return Err(Error::SilentPaymentsUnsupportedByBackend);
        }

        let mut tweaks = Vec::new();
        for height in from_height..=to_height {
            tweaks.extend(self.silent_payment_tweaks(height)?);
        }
        if tweaks.is_empty() {
            return Ok(Vec::new());
        }

        let candidates = sync.candidate_scripts(&tweaks);
        let scripts: Vec<Script> = candidates.keys().cloned().collect();
        let script_refs: Vec<&Script> = scripts.iter().collect();
        let histories = self.get_scripts_history(&script_refs)?;

        let seen: Vec<Script> = histories
            .iter()
            .zip(scripts.iter())
            .filter(|(history, _)| !history.is_empty())
            .map(|(_, script)| script.clone())
            .collect();

        let to_fetch: Vec<Txid> = sync
            .tweaks_to_scan(&tweaks, &seen)
            .into_iter()
            .map(|(txid, _)| txid)
            .collect();
        if to_fetch.is_empty() {
            return Ok(Vec::new());
        }

        self.get_transactions(&to_fetch)
    }

    /// As [`Self::scan_silent_payments()`], but keeps each transaction paired with the
    /// tweak that found it.
    ///
    /// Scanning a transaction needs `T = input_hash·A`, which is normally recovered from
    /// the scriptPubKeys its inputs spend — prevouts a light client does not have. But
    /// discovery already computed that tweak in order to derive the candidate scripts,
    /// so returning it turns an impossible lookup into a value we are holding anyway.
    #[cfg(feature = "silentpayments")]
    fn scan_silent_payments_with_tweaks(
        &self,
        sync: &crate::silentpayments::SilentPaymentSync,
        from_height: Height,
        to_height: Height,
    ) -> Result<Vec<(Transaction, crate::silentpayments::PartialTweak)>, Error> {
        if !self.capabilities().contains(&Capability::SilentPayments) {
            return Err(Error::SilentPaymentsUnsupportedByBackend);
        }

        let mut tweaks = Vec::new();
        for height in from_height..=to_height {
            tweaks.extend(self.silent_payment_tweaks(height)?);
        }
        if tweaks.is_empty() {
            return Ok(Vec::new());
        }

        let candidates = sync.candidate_scripts(&tweaks);
        let scripts: Vec<Script> = candidates.keys().cloned().collect();
        let script_refs: Vec<&Script> = scripts.iter().collect();
        let histories = self.get_scripts_history(&script_refs)?;

        let seen: Vec<Script> = histories
            .iter()
            .zip(scripts.iter())
            .filter(|(history, _)| !history.is_empty())
            .map(|(_, script)| script.clone())
            .collect();

        let hits = sync.tweaks_to_scan(&tweaks, &seen);
        if hits.is_empty() {
            return Ok(Vec::new());
        }

        let txids: Vec<Txid> = hits.iter().map(|(txid, _)| *txid).collect();
        let txs = self.get_transactions(&txids)?;

        // Re-pair by txid rather than trusting order: `get_transactions` gives no
        // ordering guarantee, and mismatching a transaction with another's tweak would
        // silently scan with the wrong shared secret and find nothing.
        let by_txid: HashMap<Txid, crate::silentpayments::PartialTweak> =
            hits.into_iter().collect();
        Ok(txs
            .into_iter()
            .filter_map(|tx| by_txid.get(&tx.txid()).map(|tweak| (tx, *tweak)))
            .collect())
    }

    /// Download and unblind the transactions
    fn download_txs<S: WolletState>(
        &self,
        history_txs_id: &HashSet<Txid>,
        scripts: &HashMap<Script, (Chain, ChildNumber, Option<BlindingPublicKey>)>,
        state: &S,
        descriptor: &WolletDescriptor,
    ) -> Result<DownloadTxResult, Error> {
        let mut txs = vec![];
        let mut unblinds = vec![];

        let txs_in_db = state.txs();
        let txs_to_download: Vec<Txid> = history_txs_id.difference(&txs_in_db).cloned().collect();

        let txs_downloaded = self.get_transactions(&txs_to_download)?;

        for tx in txs_downloaded.into_iter() {
            let txid = tx.txid();

            for (i, output) in tx.output.iter().enumerate() {
                // could be the searched script it's not yet in the cache, because created in the current run, thus it's searched also in the `scripts`
                if state.paths().contains_key(&output.script_pubkey)
                    || scripts.contains_key(&output.script_pubkey)
                {
                    let vout = i as u32;
                    let outpoint = OutPoint {
                        txid: tx.txid(),
                        vout,
                    };

                    match try_unblind(output, descriptor) {
                            Ok(unblinded) => unblinds.push((outpoint, unblinded)),
                            Err(_) => log::info!("{outpoint} cannot unblind, ignoring (could be sender messed up with the blinding process)"),
                        }
                }
            }

            txs.push((txid, tx));
        }

        Ok(DownloadTxResult { txs, unblinds })
    }

    /// Download the headers if not available in the cache
    fn download_headers<S: WolletState>(
        &self,
        history_txs_heights_plus_tip: &HashSet<Height>,
        height_blockhash: &HashMap<Height, BlockHash>,
        state: &S,
    ) -> Result<Vec<(Height, Timestamp)>, Error> {
        let mut result = vec![];
        let heights_in_db: HashSet<Height> =
            state.heights().iter().filter_map(|(_, h)| *h).collect();
        let heights_to_download: Vec<Height> = history_txs_heights_plus_tip
            .difference(&heights_in_db)
            .cloned()
            .collect();
        if !heights_to_download.is_empty() {
            for h in self.get_headers(&heights_to_download, height_blockhash)? {
                result.push((h.height, h.time))
            }

            log::debug!("{} headers_downloaded", heights_to_download.len());
        }

        Ok(result)
    }

    /// Get a transaction
    fn get_transaction(&self, txid: Txid) -> Result<Transaction, Error> {
        Ok(self
            .get_transactions(&[txid])?
            .into_iter()
            .nth(0)
            .ok_or(Error::MissingTransaction)?
            .clone())
    }

    /// Returns true if the wallet has any tx using the first gap_limit addresses (default 20)
    fn has_txs(
        &self,
        descriptor: &WolletDescriptor,
        gap_limit: Option<u32>,
    ) -> Result<bool, Error> {
        let limit = gap_limit.unwrap_or(20);

        let scripts = descriptor.derive_scripts_to_gap_limit(limit)?;
        if scripts.is_empty() {
            return Ok(false);
        }

        let histories = self.get_scripts_history(&scripts.iter().collect::<Vec<_>>())?;
        let has_any_tx = histories.into_iter().any(|history| !history.is_empty());

        Ok(has_any_tx)
    }
}

#[cfg(all(test, feature = "silentpayments"))]
mod silentpayments_tests {
    use super::*;
    use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
    use crate::silentpayments::test_fixture::SpPaymentBuilder;
    use crate::silentpayments::{PartialTweak, SilentPaymentScanMaterial, SilentPaymentSync};

    /// A backend serving canned tweaks and script histories, standing in for a server
    /// with [`Capability::SilentPayments`].
    struct MockBackend {
        tweaks: Vec<(Txid, PartialTweak)>,
        /// Scripts that exist on chain.
        on_chain: HashSet<Script>,
        txs: HashMap<Txid, Transaction>,
        supports_sp: bool,
        /// Records what was actually downloaded, to prove the narrowing works.
        fetched: std::cell::RefCell<Vec<Txid>>,
    }

    impl MockBackend {
        /// A header at `height`, enough for the scan machinery (which only reads the
        /// height and time).
        fn header_at(height: Height) -> BlockHeader {
            let mut h = crate::update::default_blockheader();
            h.height = height;
            h.time = 1_600_000_000 + height;
            h
        }
    }

    impl BlockchainBackend for MockBackend {
        fn tip(&mut self) -> Result<BlockHeader, Error> {
            Ok(Self::header_at(1))
        }
        fn broadcast(&self, _tx: &Transaction) -> Result<Txid, Error> {
            unimplemented!("not exercised")
        }
        fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<Transaction>, Error> {
            self.fetched.borrow_mut().extend_from_slice(txids);
            Ok(txids
                .iter()
                .filter_map(|t| self.txs.get(t).cloned())
                .collect())
        }
        fn get_headers(
            &self,
            heights: &[Height],
            _height_blockhash: &HashMap<Height, BlockHash>,
        ) -> Result<Vec<BlockHeader>, Error> {
            Ok(heights.iter().copied().map(Self::header_at).collect())
        }
        fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
            Ok(scripts
                .iter()
                .map(|s| {
                    if self.on_chain.contains(*s) {
                        vec![History {
                            txid: Txid::from_raw_hash(
                                <elements::hashes::sha256d::Hash as elements::hashes::Hash>::all_zeros(),
                            ),
                            height: 1,
                            block_hash: None,
                            block_timestamp: None,
                            v: 0,
                        }]
                    } else {
                        vec![]
                    }
                })
                .collect())
        }
        fn capabilities(&self) -> HashSet<Capability> {
            let mut caps = HashSet::new();
            if self.supports_sp {
                caps.insert(Capability::SilentPayments);
            }
            caps
        }
        fn silent_payment_tweaks(
            &self,
            _height: Height,
        ) -> Result<Vec<(Txid, PartialTweak)>, Error> {
            Ok(self.tweaks.clone())
        }
    }

    /// A transaction paying `keys` a silent payment, plus its tweak.
    fn paying_tx(keys: &SilentPaymentScanMaterial) -> (Transaction, PartialTweak) {
        let payment = SpPaymentBuilder::new()
            .with_inputs(&[(Data::outpoint(0x10, 0), Data::secret_key(0x31))])
            .with_value(1000)
            .without_extra_output()
            .build_for(keys);
        (payment.tx, payment.tweak)
    }

    fn keys() -> SilentPaymentScanMaterial {
        SilentPaymentScanMaterial::new(
            crate::silentpayments::SilentPaymentAccount::liquid_testnet(0),
            Data::secret_key(0x11),
            Data::secret_key(0x22).public_key(&crate::util::EC),
        )
    }

    /// A tweak whose candidates are not on chain must cost no transaction download.
    /// This is the property that makes scanning a block range affordable; without it
    /// the wallet would fetch every tweaked transaction in every block.
    #[test]
    fn unrelated_transactions_are_never_downloaded() {
        let keys = keys();
        let (tx, tweak) = paying_tx(&keys);

        // The tweak is published, but none of our candidate scripts exist on chain
        // (i.e. the payment was to somebody else).
        let backend = MockBackend {
            tweaks: vec![(tx.txid(), tweak)],
            on_chain: HashSet::new(),
            txs: [(tx.txid(), tx.clone())].into_iter().collect(),
            supports_sp: true,
            fetched: Default::default(),
        };

        let found = backend
            .scan_silent_payments(&SilentPaymentSync::new(keys), 1, 1)
            .unwrap();

        assert!(found.is_empty());
        assert!(
            backend.fetched.borrow().is_empty(),
            "no transaction should be downloaded when no candidate is on chain"
        );
    }

    /// A backend that cannot discover silent payments must say so. Reporting an empty
    /// result would be indistinguishable from "you received nothing", which is exactly
    /// how funds appear to go missing.
    #[test]
    fn backend_without_capability_errors() {
        let backend = MockBackend {
            tweaks: vec![],
            on_chain: HashSet::new(),
            txs: HashMap::new(),
            supports_sp: false,
            fetched: Default::default(),
        };

        let err = backend
            .scan_silent_payments(&SilentPaymentSync::new(keys()), 1, 1)
            .unwrap_err();
        assert!(matches!(err, Error::SilentPaymentsUnsupportedByBackend));
    }

    /// Capability and tweak support must be provided together.
    #[test]
    fn advertising_the_capability_without_implementing_tweaks_is_a_contradiction() {
        struct LyingBackend;

        impl BlockchainBackend for LyingBackend {
            fn tip(&mut self) -> Result<BlockHeader, Error> {
                unimplemented!("not exercised")
            }
            fn broadcast(&self, _tx: &Transaction) -> Result<Txid, Error> {
                unimplemented!("not exercised")
            }
            fn get_transactions(&self, _txids: &[Txid]) -> Result<Vec<Transaction>, Error> {
                unimplemented!("not exercised")
            }
            fn get_headers(
                &self,
                _heights: &[Height],
                _height_blockhash: &HashMap<Height, BlockHash>,
            ) -> Result<Vec<BlockHeader>, Error> {
                unimplemented!("not exercised")
            }
            fn get_scripts_history(
                &self,
                _scripts: &[&Script],
            ) -> Result<Vec<Vec<History>>, Error> {
                unimplemented!("not exercised")
            }
            fn capabilities(&self) -> HashSet<Capability> {
                [Capability::SilentPayments].into_iter().collect()
            }
        }

        let err = LyingBackend
            .scan_silent_payments(&SilentPaymentSync::new(keys()), 1, 1)
            .unwrap_err();
        assert!(matches!(err, Error::SilentPaymentsUnsupportedByBackend));
    }

    #[test]
    fn discovery_resumes_after_progress_and_otherwise_at_the_birthday() {
        use crate::wollet::WolletState;
        use crate::{Network, WolletBuilder};

        let build = |birthday: Option<u32>| {
            let desc: crate::WolletDescriptor =
                lwk_test_util::wollet_descriptor_string().parse().unwrap();
            let mut builder = WolletBuilder::new(Network::default_regtest(), desc)
                .with_silent_payment_material(keys());
            if let Some(birthday) = birthday {
                builder = builder.with_silent_payment_birthday(birthday);
            }
            builder.build().unwrap()
        };

        assert_eq!(
            build(Some(800_000)).silent_payments_scan_from(),
            800_000,
            "a wallet that never scanned must start at its birthday"
        );
        assert_eq!(
            build(None).silent_payments_scan_from(),
            0,
            "without a birthday the first scan starts at genesis"
        );

        for (scanned_to, expected, why) in [
            (
                900_000,
                900_001,
                "a scan must resume after what it already covered, not redo it",
            ),
            (
                700_000,
                700_001,
                "a later birthday must not open a gap the scan skips",
            ),
        ] {
            let mut wollet = build(Some(800_000));
            wollet.cache.silent_payments_scanned_to = Some(scanned_to);
            assert_eq!(wollet.silent_payments_scan_from(), expected, "{why}");
        }
    }

    /// A wallet without scan keys does not run silent-payment discovery.
    #[test]
    fn full_scan_without_scan_keys_does_no_discovery() {
        use crate::{Network, WolletBuilder};

        let desc: crate::WolletDescriptor =
            lwk_test_util::wollet_descriptor_string().parse().unwrap();
        let wollet = WolletBuilder::new(Network::default_regtest(), desc)
            .build()
            .unwrap();

        let (tx, tweak) = paying_tx(&keys());
        let mut backend = MockBackend {
            tweaks: vec![(tx.txid(), tweak)],
            on_chain: [tx.output[0].script_pubkey.clone()].into_iter().collect(),
            txs: [(tx.txid(), tx.clone())].into_iter().collect(),
            supports_sp: true,
            fetched: Default::default(),
        };

        let update = backend.full_scan(&wollet).unwrap();

        assert!(
            backend.fetched.borrow().is_empty(),
            "a wallet without scan keys must not download silent payment candidates"
        );
        if let Some(update) = update {
            assert!(
                update.silent_payments.is_none(),
                "no scan keys means no discovery claim, so a later scan starts from scratch"
            );
        }
    }
}
