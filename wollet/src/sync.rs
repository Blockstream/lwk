use crate::bitcoin::{self, Txid as BitcoinTxid};
use crate::descriptor::Chain;
use crate::elements::confidential::{Asset, Nonce, Value};
use crate::elements::encode::deserialize as elements_deserialize;
use crate::elements::{OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid};
use crate::error::Error;
use crate::store::{Height, Store, Timestamp, BATCH_SIZE};
use crate::util::EC;
use crate::{Wollet, WolletDescriptor};
use common::derive_blinding_key;
use electrum_client::bitcoin::bip32::ChildNumber;
use electrum_client::{Client, ElectrumApi, GetHistoryRes};
use elements::BlockHeader;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::sync::atomic;

#[derive(Default)]
struct DownloadTxResult {
    txs: Vec<(Txid, Transaction)>,
    unblinds: Vec<(OutPoint, TxOutSecrets)>,
}

pub fn sync(client: &Client, wollet: &mut Wollet) -> Result<bool, Error> {
    let update = scan(client, wollet)?;
    let result = update.is_some();
    if let Some(update) = update {
        apply_update(&mut wollet.store, update)?
    }
    Ok(result)
}

fn scan(client: &Client, wollet: &Wollet) -> Result<Option<Update>, Error> {
    let descriptor = wollet.wollet_descriptor();
    let store = &wollet.store;
    let mut txid_height = HashMap::new();
    let mut scripts = HashMap::new();

    let mut last_unused_external = 0;
    let mut last_unused_internal = 0;

    for descriptor in descriptor.descriptor().clone().into_single_descriptors()? {
        let mut batch_count = 0;
        let chain: Chain = (&descriptor).try_into().unwrap_or(Chain::External);
        loop {
            let batch = store.get_script_batch(batch_count, &descriptor)?;
            let scripts_bitcoin: Vec<_> = batch
                .value
                .iter()
                .map(|e| bitcoin::Script::from_bytes(e.0.as_bytes()))
                .collect();
            let result: Vec<Vec<GetHistoryRes>> =
                client.batch_script_get_history(scripts_bitcoin)?;
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
                    Chain::External => last_unused_external = 1 + max + batch_count * BATCH_SIZE,
                    Chain::Internal => last_unused_internal = 1 + max + batch_count * BATCH_SIZE,
                }
            };

            let flattened: Vec<GetHistoryRes> = result.into_iter().flatten().collect();

            if flattened.is_empty() {
                break;
            }

            for el in flattened {
                // el.height = -1 means unconfirmed with unconfirmed parents
                // el.height =  0 means unconfirmed with confirmed parents
                // but we threat those tx the same
                let height = el.height.max(0);
                let txid = Txid::from_raw_hash(el.tx_hash.to_raw_hash());
                if height == 0 {
                    txid_height.insert(txid, None);
                } else {
                    txid_height.insert(txid, Some(height as u32));
                }
            }

            batch_count += 1;
        }
    }

    let history_txs_id: HashSet<Txid> = txid_height.keys().cloned().collect();
    let new_txs = download_txs(&history_txs_id, &scripts, client, store, &descriptor)?;
    let history_txs_heights: HashSet<Height> = txid_height.values().filter_map(|e| *e).collect();
    let timestamps = download_headers(&history_txs_heights, client, store)?;

    let store_last_unused_external = store
        .cache
        .last_unused_external
        .load(atomic::Ordering::Relaxed);
    let store_last_unused_internal = store
        .cache
        .last_unused_internal
        .load(atomic::Ordering::Relaxed);

    let last_unused_changed = store_last_unused_external != last_unused_external
        || store_last_unused_internal != last_unused_internal;

    let changed = !new_txs.txs.is_empty()
        || last_unused_changed
        || !scripts.is_empty()
        || !timestamps.is_empty();

    if changed {
        tracing::debug!("something changed: !new_txs.txs.is_empty():{} last_unused_changed:{} !scripts.is_empty():{} !timestamps.is_empty():{}", !new_txs.txs.is_empty(), last_unused_changed, !scripts.is_empty(), !timestamps.is_empty() );

        let update = Update {
            new_txs,
            txid_height,
            timestamps,
            scripts,
        };
        Ok(Some(update))
    } else {
        Ok(None)
    }
}

struct Update {
    new_txs: DownloadTxResult,
    txid_height: HashMap<Txid, Option<u32>>,
    timestamps: Vec<(u32, u32)>,
    scripts: HashMap<Script, (Chain, ChildNumber)>,
}

fn apply_update(store: &mut Store, update: Update) -> Result<(), Error> {
    let Update {
        new_txs,
        mut txid_height,
        timestamps,
        scripts,
    } = update;
    store.cache.all_txs.extend(new_txs.txs);
    store.cache.unblinded.extend(new_txs.unblinds);
    let txids_unblinded: HashSet<Txid> = store.cache.unblinded.keys().map(|o| o.txid).collect();
    txid_height.retain(|txid, _| txids_unblinded.contains(txid));
    store.cache.heights.clear();
    store.cache.heights.extend(&txid_height);
    store.cache.timestamps.extend(timestamps);
    store
        .cache
        .scripts
        .extend(scripts.clone().into_iter().map(|(a, b)| (b, a)));
    store.cache.paths.extend(scripts);
    let mut last_used_internal = None;
    let mut last_used_external = None;
    for txid in txid_height.keys() {
        if let Some(tx) = store.cache.all_txs.get(txid) {
            for output in &tx.output {
                if let Some((ext_int, ChildNumber::Normal { index })) =
                    store.cache.paths.get(&output.script_pubkey)
                {
                    match ext_int {
                        Chain::External => match last_used_external {
                            None => last_used_external = Some(index),
                            Some(last) if index > last => last_used_external = Some(index),
                            _ => {}
                        },
                        Chain::Internal => match last_used_internal {
                            None => last_used_internal = Some(index),
                            Some(last) if index > last => last_used_internal = Some(index),
                            _ => {}
                        },
                    }
                }
            }
        }
    }
    if let Some(last_used_external) = last_used_external {
        store
            .cache
            .last_unused_external
            .store(last_used_external + 1, atomic::Ordering::Relaxed);
    }
    if let Some(last_used_internal) = last_used_internal {
        store
            .cache
            .last_unused_internal
            .store(last_used_internal + 1, atomic::Ordering::Relaxed);
    }
    store.flush()?;
    Ok(())
}

fn download_txs(
    history_txs_id: &HashSet<Txid>,
    scripts: &HashMap<Script, (Chain, ChildNumber)>,
    client: &Client,
    store: &Store,
    descriptor: &WolletDescriptor,
) -> Result<DownloadTxResult, Error> {
    let mut txs = vec![];
    let mut unblinds = vec![];

    let mut txs_in_db = store.cache.all_txs.keys().cloned().collect();
    let txs_to_download: Vec<&Txid> = history_txs_id.difference(&txs_in_db).collect();
    if !txs_to_download.is_empty() {
        let txs_bitcoin: Vec<BitcoinTxid> = txs_to_download
            .iter()
            .map(|t| BitcoinTxid::from_raw_hash(t.to_raw_hash()))
            .collect();
        let txs_bitcoin: Vec<&BitcoinTxid> = txs_bitcoin.iter().collect();
        let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_bitcoin)?;
        let mut txs_downloaded: Vec<Transaction> = vec![];
        for vec in txs_bytes_downloaded {
            let tx: Transaction = elements_deserialize(&vec)?;
            txs_downloaded.push(tx);
        }
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

        let txs_to_download: Vec<&Txid> = previous_txs_to_download.difference(&txs_in_db).collect();
        if !txs_to_download.is_empty() {
            let txs_bitcoin: Vec<BitcoinTxid> = txs_to_download
                .iter()
                .map(|t| BitcoinTxid::from_raw_hash(t.to_raw_hash()))
                .collect();
            let txs_bitcoin: Vec<&BitcoinTxid> = txs_bitcoin.iter().collect();
            let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_bitcoin)?;
            for vec in txs_bytes_downloaded {
                let tx: Transaction = elements_deserialize(&vec)?;
                txs.push((tx.txid(), tx));
            }
        }
        Ok(DownloadTxResult { txs, unblinds })
    } else {
        Ok(DownloadTxResult::default())
    }
}

fn download_headers(
    history_txs_heights: &HashSet<Height>,
    client: &Client,
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
        let headers_bytes_downloaded =
            client.batch_block_header_raw(heights_to_download.clone())?;
        for vec in headers_bytes_downloaded {
            let header: BlockHeader = elements::encode::deserialize(&vec)?;
            result.push((header.height, header.time));
        }
        tracing::info!("{} headers_downloaded", heights_to_download.len());
    }

    Ok(result)
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
