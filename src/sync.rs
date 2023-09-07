use crate::error::Error;
use crate::store::{Indexes, Store, BATCH_SIZE};
use electrum_client::{Client, ElectrumApi, GetHistoryRes};
use elements::bitcoin::bip32::DerivationPath;
use elements::bitcoin::secp256k1::Secp256k1;
use elements::bitcoin::{ScriptBuf as BitcoinScript, Txid as BitcoinTxid};
use elements::confidential::{Asset, Nonce, Value};
use elements::slip77::MasterBlindingKey;
use elements::{OutPoint, Script, Transaction, TxOut, TxOutSecrets, Txid};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::{HashMap, HashSet};

pub struct Syncer {
    pub store: Store,
    pub master_blinding: MasterBlindingKey,
}

#[derive(Default)]
struct DownloadTxResult {
    txs: Vec<(Txid, Transaction)>,
    unblinds: Vec<(OutPoint, TxOutSecrets)>,
}

impl Syncer {
    pub fn sync(&self, client: &Client) -> Result<bool, Error> {
        let mut history_txs_id = HashSet::new();
        let mut txid_height = HashMap::new();
        let mut scripts = HashMap::new();

        let mut last_used = Indexes::default();
        let mut wallet_chains = vec![0, 1];
        wallet_chains.shuffle(&mut thread_rng());
        for i in wallet_chains {
            let mut batch_count = 0;
            loop {
                let batch = self.store.read()?.get_script_batch(i, batch_count)?;
                let scripts_bitcoin: Vec<BitcoinScript> = batch
                    .value
                    .iter()
                    .map(|e| BitcoinScript::from(e.0.clone().into_bytes()))
                    .collect();
                let scripts_bitcoin: Vec<&_> =
                    scripts_bitcoin.iter().map(|e| e.as_script()).collect();
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
                    if i == 0 {
                        last_used.external = max + batch_count * BATCH_SIZE;
                    } else {
                        last_used.internal = max + batch_count * BATCH_SIZE;
                    }
                };

                let flattened: Vec<GetHistoryRes> = result.into_iter().flatten().collect();

                if flattened.is_empty() {
                    break;
                }
                let found_some = !flattened.is_empty();

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

                    history_txs_id.insert(txid);
                }

                // FIXME: restore looking for multiple scripts
                if found_some {
                    break;
                }

                batch_count += 1;
            }
        }

        let new_txs = self.download_txs(&history_txs_id, &scripts, &client)?;

        let store_indexes = self.store.read()?.cache.indexes.clone();

        let changed =
            if !new_txs.txs.is_empty() || store_indexes != last_used || !scripts.is_empty() {
                let mut store_write = self.store.write()?;
                store_write.cache.indexes = last_used;
                store_write.cache.all_txs.extend(new_txs.txs.into_iter());
                store_write.cache.unblinded.extend(new_txs.unblinds);

                // height map is used for the live list of transactions, since due to reorg or rbf tx
                // could disappear from the list, we clear the list and keep only the last values returned by the server
                store_write.cache.heights.clear();
                store_write.cache.heights.extend(txid_height.into_iter());

                store_write
                    .cache
                    .scripts
                    .extend(scripts.clone().into_iter().map(|(a, b)| (b, a)));
                store_write.cache.paths.extend(scripts.into_iter());
                store_write.flush()?;
                true
            } else {
                false
            };

        Ok(changed)
    }

    fn download_txs(
        &self,
        history_txs_id: &HashSet<Txid>,
        scripts: &HashMap<Script, DerivationPath>,
        client: &Client,
    ) -> Result<DownloadTxResult, Error> {
        let mut txs = vec![];
        let mut unblinds = vec![];

        let mut txs_in_db = self.store.read()?.cache.all_txs.keys().cloned().collect();
        let txs_to_download: Vec<&Txid> = history_txs_id.difference(&txs_in_db).collect();
        if !txs_to_download.is_empty() {
            let txs_bitcoin: Vec<BitcoinTxid> = txs_to_download
                .iter()
                .map(|t| BitcoinTxid::from_raw_hash(t.to_raw_hash()))
                .collect();
            let txs_bitcoin: Vec<&BitcoinTxid> = txs_bitcoin.iter().map(|t| t).collect();
            let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_bitcoin)?;
            let mut txs_downloaded: Vec<Transaction> = vec![];
            for vec in txs_bytes_downloaded {
                let tx: Transaction = elements::encode::deserialize(&vec)?;
                txs_downloaded.push(tx);
            }
            let previous_txs_to_download = HashSet::new();
            for tx in txs_downloaded.into_iter() {
                let txid = tx.txid();
                txs_in_db.insert(txid);

                for (i, output) in tx.output.iter().enumerate() {
                    // could be the searched script it's not yet in the store, because created in the current run, thus it's searched also in the `scripts`
                    if self
                        .store
                        .read()?
                        .cache
                        .paths
                        .contains_key(&output.script_pubkey)
                        || scripts.contains_key(&output.script_pubkey)
                    {
                        let vout = i as u32;
                        let outpoint = OutPoint {
                            txid: tx.txid(),
                            vout,
                        };

                        match self.try_unblind(output.clone()) {
                            Ok(unblinded) => unblinds.push((outpoint, unblinded)),
                            Err(_) => log::info!("{} cannot unblind, ignoring (could be sender messed up with the blinding process)", outpoint),
                        }
                    }
                }

                txs.push((txid, tx));
            }

            let txs_to_download: Vec<&Txid> =
                previous_txs_to_download.difference(&txs_in_db).collect();
            if !txs_to_download.is_empty() {
                let txs_bitcoin: Vec<BitcoinTxid> = txs_to_download
                    .iter()
                    .map(|t| BitcoinTxid::from_raw_hash(t.to_raw_hash()))
                    .collect();
                let txs_bitcoin: Vec<&BitcoinTxid> = txs_bitcoin.iter().map(|t| t).collect();
                let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_bitcoin)?;
                for vec in txs_bytes_downloaded {
                    let tx: Transaction = elements::encode::deserialize(&vec)?;
                    txs.push((tx.txid(), tx));
                }
            }
            Ok(DownloadTxResult { txs, unblinds })
        } else {
            Ok(DownloadTxResult::default())
        }
    }

    pub fn try_unblind(&self, output: TxOut) -> Result<TxOutSecrets, Error> {
        match (output.asset, output.value, output.nonce) {
            (Asset::Confidential(_), Value::Confidential(_), Nonce::Confidential(_)) => {
                // TODO: use a shared ctx
                let secp = Secp256k1::new();
                let receiver_sk = self
                    .master_blinding
                    .derive_blinding_key(&output.script_pubkey);
                // TODO: implement UnblindError and remove Generic
                let txout_secrets = output
                    .unblind(&secp, receiver_sk)
                    .map_err(|_| Error::Generic("UnblindError".into()))?;

                Ok(txout_secrets)
            }
            _ => Err(Error::Generic(
                "received unconfidential or null asset/value/nonce".into(),
            )),
        }
    }
}
