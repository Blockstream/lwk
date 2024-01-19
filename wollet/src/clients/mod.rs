use ::electrum_client::GetHistoryRes;
use elements::{BlockHeader, Transaction, Txid};
use std::{borrow::Borrow, convert::TryInto, sync::atomic};
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

    fn get_transactions<I>(&self, txids: I) -> Result<Vec<Transaction>, Error>
    where
        I: IntoIterator + Clone,
        I::Item: Borrow<Txid>;

    fn get_headers<I>(&self, heights: I) -> Result<Vec<BlockHeader>, Error>
    where
        I: IntoIterator + Clone,
        I::Item: Borrow<u32>;

    fn get_scripts_history<'s, I>(&self, scripts: I) -> Result<Vec<Vec<GetHistoryRes>>, Error>
    where
        I: IntoIterator + Clone,
        I::Item: Borrow<&'s Script>;

    fn full_scan(&mut self, wollet: &Wollet) -> Result<Option<Update>, Error> {
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

                let result: Vec<Vec<GetHistoryRes>> =
                    self.get_scripts_history(batch.value.iter().map(|e| &e.0))?;
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
        let new_txs = self.download_txs(&history_txs_id, &scripts, store, &descriptor)?;
        let history_txs_heights: HashSet<Height> =
            txid_height.values().filter_map(|e| *e).collect();
        let timestamps = self.download_headers(&history_txs_heights, store)?;

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
        let txs_to_download: Vec<&Txid> = history_txs_id.difference(&txs_in_db).collect();
        if !txs_to_download.is_empty() {
            let txs_downloaded = self.get_transactions(txs_to_download)?;

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

            let txs_to_download: Vec<&Txid> =
                previous_txs_to_download.difference(&txs_in_db).collect();
            if !txs_to_download.is_empty() {
                for tx in self.get_transactions(txs_to_download)? {
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
            for h in self.get_headers(&heights_to_download)? {
                result.push((h.height, h.time))
            }

            tracing::info!("{} headers_downloaded", heights_to_download.len());
        }

        Ok(result)
    }
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
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = signer::SwSigner::new(mnemonic, false).unwrap();
        let script_variant = common::Singlesig::Wpkh;
        let blinding_variant = common::DescriptorBlindingKey::Slip77;
        let desc_str =
            common::singlesig_desc(&signer, script_variant, blinding_variant, false).unwrap();

        let tempdir1 = tempfile::tempdir().unwrap();
        let mut wollet1 = crate::Wollet::new(
            ElementsNetwork::LiquidTestnet,
            &tempdir1.path().display().to_string(),
            &desc_str,
        )
        .unwrap();

        let tempdir2 = tempfile::tempdir().unwrap();
        let mut wollet2 = crate::Wollet::new(
            ElementsNetwork::LiquidTestnet,
            &tempdir2.path().display().to_string(),
            &desc_str,
        )
        .unwrap();

        let mut electrum_client =
            ElectrumClient::new(&ElectrumUrl::new("blockstream.info:465", true, true)).unwrap();
        let mut esplora_client = EsploraClient::new("https://blockstream.info/liquidtestnet/api");

        let start = Instant::now();
        let a = electrum_client.full_scan(&wollet1).unwrap();
        wollet1.apply_update(a.unwrap()).unwrap();
        println!("{}", start.elapsed().as_secs());

        let b = esplora_client.full_scan(&wollet2).unwrap();
        wollet2.apply_update(b.unwrap()).unwrap();

        assert_eq!(wollet1.balance().unwrap(), wollet2.balance().unwrap());
    }
}
