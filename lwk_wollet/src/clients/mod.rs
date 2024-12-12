use crate::{
    store::{Height, Timestamp, BATCH_SIZE},
    update::{DownloadTxResult, Update},
    wollet::WolletState,
    Chain, Error, WolletDescriptor, EC,
};
use elements::{
    bitcoin::bip32::ChildNumber,
    confidential::{Asset, Nonce, Value},
    OutPoint, Script, TxOut, TxOutSecrets,
};
use elements::{BlockHash, BlockHeader, Transaction, Txid};
use lwk_common::derive_blinding_key;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    ops::{Index, IndexMut},
};

#[cfg(not(target_arch = "wasm32"))]
pub mod blocking;

pub mod asyncr;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LastUnused {
    pub internal: u32,
    pub external: u32,
}

impl Index<Chain> for LastUnused {
    type Output = u32;

    fn index(&self, index: Chain) -> &Self::Output {
        match index {
            Chain::External => &self.external,
            Chain::Internal => &self.internal,
        }
    }
}

impl IndexMut<Chain> for LastUnused {
    fn index_mut(&mut self, index: Chain) -> &mut Self::Output {
        match index {
            Chain::External => &mut self.external,
            Chain::Internal => &mut self.internal,
        }
    }
}

/// Data processed after a "get history" call
#[derive(Debug, PartialEq, Eq, Default)]
pub struct Data {
    pub txid_height: HashMap<Txid, Option<Height>>,
    pub scripts: HashMap<Script, (Chain, ChildNumber)>,
    pub last_unused: LastUnused,
    pub height_blockhash: HashMap<Height, BlockHash>,
    pub height_timestamp: HashMap<Height, Timestamp>,
}

/// Capabilities that can be supported by a [`BlockchainBackend`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Can interfact with a Waterfalls data source
    Waterfalls,
}

/// Trait implemented by types that can fetch data from a blockchain data source.
pub trait BlockchainBackend {
    /// Get the blockchain latest block
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

    fn get_history<S: WolletState>(
        &mut self,
        descriptor: &WolletDescriptor,
        state: &S,
    ) -> Result<Data, Error> {
        let mut data = Data::default();

        for descriptor in descriptor.descriptor().clone().into_single_descriptors()? {
            let mut batch_count = 0;
            let chain: Chain = (&descriptor).try_into().unwrap_or(Chain::External);
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

                if flattened.is_empty() {
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
            }
        }
        Ok(data)
    }

    fn get_history_waterfalls<S: WolletState>(
        &mut self,
        _descriptor: &WolletDescriptor,
        _state: &S,
    ) -> Result<Data, Error> {
        Err(Error::WaterfallsUnimplemented)
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet
    fn full_scan<S: WolletState>(&mut self, state: &S) -> Result<Option<Update>, Error> {
        let descriptor = state.descriptor();

        let Data {
            txid_height,
            scripts,
            last_unused,
            height_blockhash,
            height_timestamp: _height_timestamp,
        } = if self.capabilities().contains(&Capability::Waterfalls) {
            match self.get_history_waterfalls(&descriptor, state) {
                Ok(d) => d,
                Err(Error::UsingWaterfallsWithElip151) => self.get_history(&descriptor, state)?,
                Err(e) => return Err(e),
            }
        } else {
            self.get_history(&descriptor, state)?
        };

        let tip = self.tip()?;

        let history_txs_id: HashSet<Txid> = txid_height.keys().cloned().collect();
        let new_txs = self.download_txs(&history_txs_id, &scripts, state, &descriptor)?;
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

            let update = Update {
                wollet_status,
                new_txs,
                txid_height_new,
                txid_height_delete,
                timestamps,
                scripts,
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
        scripts: &HashMap<Script, (Chain, ChildNumber)>,
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

                    match try_unblind(output.clone(), descriptor) {
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
}

#[derive(Debug, Clone, Deserialize)]
/// Position of a transaction involving a certain script
pub struct History {
    /// Transaction ID
    pub txid: Txid,

    /// Confirmation height of txid
    ///
    /// -1 means unconfirmed with unconfirmed parents
    ///  0 means unconfirmed with confirmed parents
    pub height: i32,

    /// The block hash of the block including the transaction, if available
    pub block_hash: Option<BlockHash>,

    /// The block hash of the block including the transaction, if available
    pub block_timestamp: Option<Timestamp>,
}

pub fn try_unblind(output: TxOut, descriptor: &WolletDescriptor) -> Result<TxOutSecrets, Error> {
    match (output.asset, output.value, output.nonce) {
        (Asset::Confidential(_), Value::Confidential(_), Nonce::Confidential(_)) => {
            let receiver_sk = derive_blinding_key(descriptor.as_ref(), &output.script_pubkey)
                .ok_or_else(|| Error::MissingPrivateBlindingKey)?;
            let txout_secrets = output.unblind(&EC, receiver_sk)?;

            Ok(txout_secrets)
        }
        _ => Err(Error::Generic(
            "received unconfidential or null asset/value/nonce".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    /*
    use std::time::Instant;

    use crate::{
        clients::esplora_client::EsploraClient, BlockchainBackend, ElectrumClient, ElectrumUrl,
        ElementsNetwork,
    };

    #[test]
    #[ignore = "test with prod servers"]
    fn esplora_electrum_compare() {
        lwk_test_util::init_logging();

        let desc_str = lwk_test_util::TEST_DESCRIPTOR;

        let urls = [
            LIQUID_TESTNET_SOCKET,
            "https://blockstream.info/liquidtestnet/api",
            "https://liquid.network/liquidtestnet/api",
        ];

        let vec: Vec<Box<dyn BlockchainBackend>> = vec![
            Box::new(ElectrumClient::new(&ElectrumUrl::new(urls[0], true, true)).unwrap()),
            Box::new(EsploraClient::new(urls[1])),
            Box::new(EsploraClient::new(urls[2])),
        ];

        let mut prec = None;

        for (i, mut bb) in vec.into_iter().enumerate() {
            let tempdir = tempfile::tempdir().unwrap();
            let desc = desc_str.parse().unwrap();
            let mut wollet =
                crate::Wollet::with_fs_persist(ElementsNetwork::LiquidTestnet, desc, &tempdir)
                    .unwrap();

            let start = Instant::now();
            let first_update = bb.full_scan(&wollet).unwrap().unwrap();
            wollet.apply_update(first_update.clone()).unwrap();

            let balance = wollet.balance().unwrap();

            if let Some(prec) = prec.as_ref() {
                assert_eq!(&balance, prec);
            }
            prec = Some(balance);

            log::info!(
                "first run: {}: {:.2}s",
                urls[i],
                start.elapsed().as_secs_f64()
            );

            let start = Instant::now();
            let second_update = bb.full_scan(&wollet.state()).unwrap();
            if let Some(update) = second_update {
                // the tip could have been updated, checking no new tx have been found
                assert!(update.new_txs.unblinds.is_empty());
                assert!(update.scripts.is_empty());
                assert!(update.timestamps.is_empty());
                assert!(update.txid_height_new.is_empty());
                assert!(update.txid_height_delete.is_empty());
                assert_ne!(update.tip, first_update.tip);
            }
            log::info!(
                "second run: {}: {:.2}s",
                urls[i],
                start.elapsed().as_secs_f64()
            );
        }
    }
    * */
}
