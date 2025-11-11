//! Blocking clients to fetch data from the Blockchain.

use crate::{
    clients::{create_dummy_tx, try_unblind},
    store::{Height, Timestamp, BATCH_SIZE},
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
pub use esplora::EsploraClient;

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
                let batch = state.get_script_batch(batch_count, &descriptor)?;

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

                if !descriptor.descriptor.has_wildcard() {
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
        let mut new_txs = self.download_txs(&history_txs_id, &scripts, state, &descriptor)?;

        if self.utxo_only() {
            let tx = create_dummy_tx(&unspent, &new_txs);
            new_txs.txs.push((tx.txid(), tx));
        }

        let history_txs_heights_plus_tip: HashSet<Height> = txid_height
            .values()
            .filter_map(|e| *e)
            .chain(std::iter::once(tip.height))
            .collect();
        let timestamps =
            self.download_headers(&history_txs_heights_plus_tip, &height_blockhash, state)?;

        let store_last_unused_external = state.last_unused()[Chain::External];
        let store_last_unused_internal = state.last_unused()[Chain::Internal];

        let last_unused_changed = store_last_unused_external != last_unused.external
            || store_last_unused_internal != last_unused.internal;

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
                    (*chain, *child, script.clone(), Some(*blinding_pubkey))
                })
                .collect();

            let update = Update {
                version: 2,
                wollet_status,
                new_txs,
                txid_height_new,
                txid_height_delete,
                timestamps,
                scripts_with_blinding_pubkey,
                tip,
            };
            Ok(Some(update))
        } else {
            Ok(None)
        }
    }

    /// Download and unblind the transactions
    fn download_txs<S: WolletState>(
        &self,
        history_txs_id: &HashSet<Txid>,
        scripts: &HashMap<Script, (Chain, ChildNumber, BlindingPublicKey)>,
        state: &S,
        descriptor: &WolletDescriptor,
    ) -> Result<DownloadTxResult, Error> {
        let mut txs = vec![];
        let mut unblinds = vec![];

        let mut txs_in_db = state.txs().clone();
        let txs_to_download: Vec<Txid> = history_txs_id.difference(&txs_in_db).cloned().collect();

        let txs_downloaded = self.get_transactions(&txs_to_download)?;

        for tx in txs_downloaded.into_iter() {
            let txid = tx.txid();
            txs_in_db.insert(txid);

            for (i, output) in tx.output.iter().enumerate() {
                // could be the searched script it's not yet in the store, because created in the current run, thus it's searched also in the `scripts`
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
                            Err(_) => log::info!("{} cannot unblind, ignoring (could be sender messed up with the blinding process)", outpoint),
                        }
                }
            }

            txs.push((txid, tx));
        }

        Ok(DownloadTxResult { txs, unblinds })
    }

    /// Download the headers if not available in the store
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
}
