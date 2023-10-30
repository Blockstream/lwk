use crate::bitcoin::{ScriptBuf as BitcoinScript, Txid as BitcoinTxid};
use crate::elements::confidential::{Asset, Nonce, Value};
use crate::elements::encode::deserialize as elements_deserialize;
use crate::elements::{OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid};
use crate::error::Error;
use crate::store::{Store, BATCH_SIZE};
use crate::util::EC;
use electrum_client::bitcoin::bip32::ChildNumber;
use electrum_client::{Client, ElectrumApi, GetHistoryRes};
use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use pset_common::derive_blinding_key;
use std::collections::{HashMap, HashSet};
use std::sync::atomic;

#[derive(Default)]
struct DownloadTxResult {
    txs: Vec<(Txid, Transaction)>,
    unblinds: Vec<(OutPoint, TxOutSecrets)>,
}

pub fn sync(
    client: &Client,
    store: &mut Store,
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
) -> Result<bool, Error> {
    let mut txid_height = HashMap::new();
    let mut scripts = HashMap::new();

    let mut last_unused = 0;

    let mut batch_count = 0;
    loop {
        let batch = store.get_script_batch(batch_count)?;
        let scripts_bitcoin: Vec<BitcoinScript> = batch
            .value
            .iter()
            .map(|e| BitcoinScript::from(e.0.clone().into_bytes()))
            .collect();
        let scripts_bitcoin: Vec<&_> = scripts_bitcoin.iter().map(|e| e.as_script()).collect();
        let result: Vec<Vec<GetHistoryRes>> = client.batch_script_get_history(scripts_bitcoin)?;
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
            last_unused = 1 + max + batch_count * BATCH_SIZE;
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

    let history_txs_id: HashSet<Txid> = txid_height.keys().cloned().collect();
    let new_txs = download_txs(&history_txs_id, &scripts, client, store, descriptor)?;

    let store_last_unused = store.cache.last_unused.load(atomic::Ordering::Relaxed);
    let last_unused_changed = store_last_unused != last_unused;

    let changed = if !new_txs.txs.is_empty() || last_unused_changed || !scripts.is_empty() {
        store.cache.all_txs.extend(new_txs.txs);
        store.cache.unblinded.extend(new_txs.unblinds);

        // Prune transactions that do not contain any input or output that we have unblinded
        let txids_unblinded: HashSet<Txid> = store.cache.unblinded.keys().map(|o| o.txid).collect();
        txid_height.retain(|txid, _| txids_unblinded.contains(txid));

        // Find the last used index in an output that we can unblind
        let mut last_used = None;
        for txid in txid_height.keys() {
            if let Some(tx) = store.cache.all_txs.get(txid) {
                for output in &tx.output {
                    if let Some(ChildNumber::Normal { index }) =
                        store.cache.paths.get(&output.script_pubkey)
                    {
                        match last_used {
                            None => last_used = Some(index),
                            Some(last) if index > last => last_used = Some(index),
                            _ => {}
                        }
                    }
                }
            }
        }
        if let Some(last_used) = last_used {
            store
                .cache
                .last_unused
                .store(last_used + 1, atomic::Ordering::Relaxed);
        }

        // height map is used for the live list of transactions, since due to reorg or rbf tx
        // could disappear from the list, we clear the list and keep only the last values returned by the server
        store.cache.heights.clear();
        store.cache.heights.extend(txid_height);

        store
            .cache
            .scripts
            .extend(scripts.clone().into_iter().map(|(a, b)| (b, a)));
        store.cache.paths.extend(scripts);
        store.flush()?;
        true
    } else {
        false
    };

    Ok(changed)
}

fn download_txs(
    history_txs_id: &HashSet<Txid>,
    scripts: &HashMap<Script, ChildNumber>,
    client: &Client,
    store: &mut Store,
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
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

pub fn try_unblind(
    output: TxOut,
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
) -> Result<TxOutSecrets, Error> {
    match (output.asset, output.value, output.nonce) {
        (Asset::Confidential(_), Value::Confidential(_), Nonce::Confidential(_)) => {
            let receiver_sk = derive_blinding_key(descriptor, &output.script_pubkey)
                .ok_or_else(|| Error::MissingPrivateBlindingKey)?;
            let txout_secrets = output.unblind(&EC, receiver_sk)?;

            Ok(txout_secrets)
        }
        _ => Err(Error::Generic(
            "received unconfidential or null asset/value/nonce".into(),
        )),
    }
}
