use crate::error::Error;
use crate::store::{Store, BATCH_SIZE};
use crate::util::EC;
use electrum_client::bitcoin::bip32::ChildNumber;
use electrum_client::{Client, ElectrumApi, GetHistoryRes};
use elements_miniscript::confidential::bare::tweak_private_key;
use elements_miniscript::confidential::Key;
use elements_miniscript::descriptor::DescriptorSecretKey;
use elements_miniscript::elements::bitcoin::secp256k1::SecretKey;
use elements_miniscript::elements::bitcoin::{ScriptBuf as BitcoinScript, Txid as BitcoinTxid};
use elements_miniscript::elements::confidential::{Asset, Nonce, Value};
use elements_miniscript::elements::encode::deserialize as elements_deserialize;
use elements_miniscript::elements::{OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid};
use elements_miniscript::DefiniteDescriptorKey;
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
    blinding_key: Key<DefiniteDescriptorKey>,
) -> Result<bool, Error> {
    let mut txid_height = HashMap::new();
    let mut scripts = HashMap::new();

    let mut last_used = 0;

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
            last_used = max + batch_count * BATCH_SIZE;
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
    let new_txs = download_txs(&history_txs_id, &scripts, client, store, &blinding_key)?;

    let store_indexes = store.cache.last_index.load(atomic::Ordering::Relaxed);

    let changed = if !new_txs.txs.is_empty() || store_indexes != last_used || !scripts.is_empty() {
        store
            .cache
            .last_index
            .store(last_used, atomic::Ordering::Relaxed);
        store.cache.all_txs.extend(new_txs.txs);
        store.cache.unblinded.extend(new_txs.unblinds);

        // Prune transactions that do not contain any input or output that we have unblinded
        let txids_unblinded: HashSet<Txid> = store.cache.unblinded.keys().map(|o| o.txid).collect();
        txid_height.retain(|txid, _| txids_unblinded.contains(txid));

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
    blinding_key: &Key<DefiniteDescriptorKey>,
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

                    match try_unblind(output.clone(), blinding_key) {
                            Ok(unblinded) => unblinds.push((outpoint, unblinded)),
                            Err(_) => log::info!("{} cannot unblind, ignoring (could be sender messed up with the blinding process)", outpoint),
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

fn derive_blinding_key(
    script_pubkey: &Script,
    descriptor_blinding_key: &Key<DefiniteDescriptorKey>,
) -> SecretKey {
    match descriptor_blinding_key {
        Key::Slip77(k) => k.blinding_private_key(script_pubkey),
        Key::View(DescriptorSecretKey::XPrv(dxk)) => {
            let k = dxk.xkey.to_priv();
            tweak_private_key(&EC, script_pubkey, &k.inner)
        }
        Key::View(DescriptorSecretKey::Single(k)) => {
            tweak_private_key(&EC, script_pubkey, &k.key.inner)
        }
        _ => panic!("Unsupported descriptor blinding key"),
    }
}

pub fn try_unblind(
    output: TxOut,
    blinding_key: &Key<DefiniteDescriptorKey>,
) -> Result<TxOutSecrets, Error> {
    match (output.asset, output.value, output.nonce) {
        (Asset::Confidential(_), Value::Confidential(_), Nonce::Confidential(_)) => {
            let receiver_sk = derive_blinding_key(&output.script_pubkey, blinding_key);
            let txout_secrets = output.unblind(&EC, receiver_sk)?;

            Ok(txout_secrets)
        }
        _ => Err(Error::Generic(
            "received unconfidential or null asset/value/nonce".into(),
        )),
    }
}
