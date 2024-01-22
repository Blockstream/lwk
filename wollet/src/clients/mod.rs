use elements::{BlockHash, BlockHeader, Transaction, Txid};
use std::{convert::TryInto, sync::atomic};
pub(crate) mod electrum_client;
use crate::{
    store::{Height, Store, Timestamp, BATCH_SIZE},
    update::{DownloadTxResult, Update},
    Chain, Error, Wollet, WolletDescriptor, EC,
};
use common::derive_blinding_key;
use elements::{
    bitcoin::bip32::ChildNumber,
    confidential::{Asset, Nonce, Value},
    OutPoint, Script, TxOut, TxOutSecrets,
};
use std::collections::{HashMap, HashSet};

#[cfg(feature = "esplora")]
pub(crate) mod esplora_client;

pub trait BlockchainBackend {
    fn tip(&mut self) -> Result<BlockHeader, Error>;
    fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error>;

    fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<Transaction>, Error>;

    fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<BlockHeader>, Error>;

    fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error>;

    fn full_scan(&mut self, wollet: &Wollet) -> Result<Option<Update>, Error> {
        let descriptor = wollet.wollet_descriptor();
        let store = &wollet.store;
        let mut txid_height = HashMap::new();
        let mut scripts = HashMap::new();

        let mut last_unused_external = 0;
        let mut last_unused_internal = 0;
        let mut height_blockhash = HashMap::new();

        for descriptor in descriptor.descriptor().clone().into_single_descriptors()? {
            let mut batch_count = 0;
            let chain: Chain = (&descriptor).try_into().unwrap_or(Chain::External);
            loop {
                let batch = store.get_script_batch(batch_count, &descriptor)?;

                let s: Vec<_> = batch.value.iter().map(|e| &e.0).collect();
                let result: Vec<Vec<History>> = self.get_scripts_history(&s)?;
                if !batch.cached {
                    scripts.extend(batch.value);
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
                            last_unused_external = 1 + max + batch_count * BATCH_SIZE
                        }
                        Chain::Internal => {
                            last_unused_internal = 1 + max + batch_count * BATCH_SIZE
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
                        txid_height.insert(txid, None);
                    } else {
                        txid_height.insert(txid, Some(height as u32));
                        if let Some(block_hash) = el.block_hash {
                            height_blockhash.insert(height as u32, block_hash);
                        }
                    }
                }

                batch_count += 1;
            }
        }

        let history_txs_id: HashSet<Txid> = txid_height.keys().cloned().collect();
        let new_txs = self.download_txs(&history_txs_id, &scripts, store, &descriptor)?;
        let history_txs_heights: HashSet<Height> =
            txid_height.values().filter_map(|e| *e).collect();
        let timestamps = self.download_headers(&history_txs_heights, &height_blockhash, store)?;

        let store_last_unused_external = store
            .cache
            .last_unused_external
            .load(atomic::Ordering::Relaxed);
        let store_last_unused_internal = store
            .cache
            .last_unused_internal
            .load(atomic::Ordering::Relaxed);

        let tip = self.tip()?;

        let last_unused_changed = store_last_unused_external != last_unused_external
            || store_last_unused_internal != last_unused_internal;

        let changed = !new_txs.txs.is_empty()
            || last_unused_changed
            || !scripts.is_empty()
            || !timestamps.is_empty()
            || store.cache.tip != (tip.height, tip.block_hash());

        if changed {
            tracing::debug!("something changed: !new_txs.txs.is_empty():{} last_unused_changed:{} !scripts.is_empty():{} !timestamps.is_empty():{}", !new_txs.txs.is_empty(), last_unused_changed, !scripts.is_empty(), !timestamps.is_empty() );

            let update = Update {
                new_txs,
                txid_height,
                timestamps,
                scripts,
                tip,
            };
            Ok(Some(update))
        } else {
            Ok(None)
        }
    }

    fn download_txs(
        &self,
        history_txs_id: &HashSet<Txid>,
        scripts: &HashMap<Script, (Chain, ChildNumber)>,
        store: &Store,
        descriptor: &WolletDescriptor,
    ) -> Result<DownloadTxResult, Error> {
        let mut txs = vec![];
        let mut unblinds = vec![];

        let mut txs_in_db = store.cache.all_txs.keys().cloned().collect();
        let txs_to_download: Vec<Txid> = history_txs_id.difference(&txs_in_db).cloned().collect();
        if !txs_to_download.is_empty() {
            let txs_downloaded = self.get_transactions(&txs_to_download)?;

            let previous_txs_to_download = HashSet::new();
            for tx in txs_downloaded.into_iter() {
                let txid = tx.txid();
                txs_in_db.insert(txid);

                for (i, output) in tx.output.iter().enumerate() {
                    // could be the searched script it's not yet in the store, because created in the current run, thus it's searched also in the `scripts`
                    if store.cache.paths.contains_key(&output.script_pubkey)
                        || scripts.contains_key(&output.script_pubkey)
                    {
                        let vout = i as u32;
                        let outpoint = OutPoint {
                            txid: tx.txid(),
                            vout,
                        };

                        match try_unblind(output.clone(), descriptor) {
                            Ok(unblinded) => unblinds.push((outpoint, unblinded)),
                            Err(_) => tracing::info!("{} cannot unblind, ignoring (could be sender messed up with the blinding process)", outpoint),
                        }
                    }
                }

                // FIXME: If no output is unblinded we should ignore this transaction,
                // also we should not insert this in `heights`.
                txs.push((txid, tx));
            }

            let txs_to_download: Vec<Txid> = previous_txs_to_download
                .difference(&txs_in_db)
                .cloned()
                .collect();
            if !txs_to_download.is_empty() {
                for tx in self.get_transactions(&txs_to_download)? {
                    txs.push((tx.txid(), tx));
                }
            }
            Ok(DownloadTxResult { txs, unblinds })
        } else {
            Ok(DownloadTxResult::default())
        }
    }

    fn download_headers(
        &self,
        history_txs_heights: &HashSet<Height>,
        height_blockhash: &HashMap<Height, BlockHash>,
        store: &Store,
    ) -> Result<Vec<(Height, Timestamp)>, Error> {
        let mut result = vec![];
        let heights_in_db: HashSet<Height> =
            store.cache.heights.iter().filter_map(|(_, h)| *h).collect();
        let heights_to_download: Vec<Height> = history_txs_heights
            .difference(&heights_in_db)
            .cloned()
            .collect();
        if !heights_to_download.is_empty() {
            for h in self.get_headers(&heights_to_download, height_blockhash)? {
                result.push((h.height, h.time))
            }

            tracing::info!("{} headers_downloaded", heights_to_download.len());
        }

        Ok(result)
    }
}

pub struct History {
    txid: Txid,
    height: i32,
    block_hash: Option<BlockHash>,
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
    use std::time::Instant;

    use crate::{
        clients::esplora_client::EsploraClient, BlockchainBackend, ElectrumClient, ElectrumUrl,
        ElementsNetwork,
    };

    #[test]
    #[ignore]
    fn esplora_electrum_compare() {
        test_util::init_logging();

        // TODO use a watch-only descriptor preloaded with tens of transactions, compare results at the end
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = signer::SwSigner::new(mnemonic, false).unwrap();
        let script_variant = common::Singlesig::Wpkh;
        let blinding_variant = common::DescriptorBlindingKey::Slip77;
        let desc_str =
            common::singlesig_desc(&signer, script_variant, blinding_variant, false).unwrap();

        let urls = [
            "blockstream.info:465",
            "https://blockstream.info/liquidtestnet/api",
            "https://liquid.network/liquidtestnet/api",
        ];

        let vec: Vec<Box<dyn BlockchainBackend>> = vec![
            Box::new(ElectrumClient::new(&ElectrumUrl::new(urls[0], true, true)).unwrap()),
            Box::new(EsploraClient::new(urls[1])),
            Box::new(EsploraClient::new(urls[2])),
        ];

        for (i, mut bb) in vec.into_iter().enumerate() {
            let wollet = tempfile::tempdir().unwrap();
            let mut wollet = crate::Wollet::new(
                ElementsNetwork::LiquidTestnet,
                &wollet.path().display().to_string(),
                &desc_str,
            )
            .unwrap();

            let start = Instant::now();
            let a = bb.full_scan(&wollet).unwrap();
            wollet.apply_update(a.unwrap()).unwrap();
            tracing::info!("first run: {}: {}s", urls[i], start.elapsed().as_secs());

            let start = Instant::now();
            let _ = bb.full_scan(&wollet).unwrap();
            tracing::info!("second run: {}: {}s", urls[i], start.elapsed().as_secs());
        }
    }
}
