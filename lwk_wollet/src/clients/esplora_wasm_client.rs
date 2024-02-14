//! NOTE This module is temporary, as soon we make the other clients async this will be merged in
//! the standard esplora client of which contain a lot of duplicated code.

use super::{try_unblind, History};
use crate::{
    store::{Height, Store, Timestamp, BATCH_SIZE},
    update::DownloadTxResult,
    Chain, Error, Update, Wollet, WolletDescriptor,
};
use elements::{bitcoin::bip32::ChildNumber, OutPoint};
use elements::{
    encode::Decodable,
    hashes::{hex::FromHex, sha256, Hash},
    hex::ToHex,
    pset::serialize::Serialize,
    BlockHash, Script, Txid,
};
use reqwest::Response;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::atomic,
};

#[derive(Debug)]
/// A blockchain backend implementation based on the
/// [esplora HTTP API](https://github.com/blockstream/esplora/blob/master/API.md)
pub struct EsploraWasmClient {
    base_url: String,
    tip_hash_url: String,
    broadcast_url: String,
}

impl EsploraWasmClient {
    pub fn new(url: &str) -> Self {
        Self {
            base_url: url.to_string(),
            tip_hash_url: format!("{url}/blocks/tip/hash"),
            broadcast_url: format!("{url}/tx"),
        }
    }

    async fn last_block_hash(&mut self) -> Result<elements::BlockHash, crate::Error> {
        let response = get_with_retry(&self.tip_hash_url).await?;
        Ok(BlockHash::from_str(&response.text().await?)?)
    }

    pub async fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        let last_block_hash = self.last_block_hash().await?;

        let header_url = format!("{}/block/{}/header", self.base_url, last_block_hash);
        let response = get_with_retry(&header_url).await?;
        let header_bytes = Vec::<u8>::from_hex(&response.text().await?)?;

        let header = elements::BlockHeader::consensus_decode(&header_bytes[..])?;
        Ok(header)
    }

    pub async fn broadcast(
        &self,
        tx: &elements::Transaction,
    ) -> Result<elements::Txid, crate::Error> {
        let tx_bytes = tx.serialize();
        let client = reqwest::Client::new();
        let response = client
            .post(&self.broadcast_url)
            .body(tx_bytes)
            .send()
            .await?;
        let txid = elements::Txid::from_str(&response.text().await?)?;
        Ok(txid)
    }

    async fn get_transactions(&self, txids: &[Txid]) -> Result<Vec<elements::Transaction>, Error> {
        let mut result = vec![];
        for txid in txids.iter() {
            let tx_url = format!("{}/tx/{}/raw", self.base_url, txid);
            let response = get_with_retry(&tx_url).await?;
            let tx = elements::Transaction::consensus_decode(&response.bytes().await?[..])?;
            result.push(tx);
        }
        Ok(result)
    }

    async fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        let mut result = vec![];
        for height in heights.iter() {
            let block_hash = match height_blockhash.get(height) {
                Some(block_hash) => *block_hash,
                None => {
                    let block_height = format!("{}/block-height/{}", self.base_url, height);
                    let response = get_with_retry(&block_height).await?;
                    BlockHash::from_str(&response.text().await?)?
                }
            };

            let block_header = format!("{}/block/{}/header", self.base_url, block_hash);
            let response = get_with_retry(&block_header).await?;
            let header_bytes = Vec::<u8>::from_hex(&response.text().await?)?;

            let header = elements::BlockHeader::consensus_decode(&header_bytes[..])?;

            result.push(header);
        }
        Ok(result)
    }

    // examples:
    // https://blockstream.info/liquidtestnet/api/address/tex1qntw9m0j2e93n84x975t47ddhgkzx3x8lhfv2nj/txs
    // https://blockstream.info/liquidtestnet/api/scripthash/b50a2a798d876db54acfa0d8dfdc49154ea8defed37b225ec4c9ec7415358ba3/txs
    async fn get_scripts_history(&self, scripts: &[&Script]) -> Result<Vec<Vec<History>>, Error> {
        let mut result: Vec<_> = vec![];
        for script in scripts.iter() {
            let script = elements::bitcoin::Script::from_bytes(script.as_bytes());
            let script_hash = sha256::Hash::hash(script.as_bytes()).to_byte_array();
            let url = format!("{}/scripthash/{}/txs", self.base_url, script_hash.to_hex());
            // TODO must handle paging -> https://github.com/blockstream/esplora/blob/master/API.md#addresses
            let response = get_with_retry(&url).await?;
            let json: Vec<EsploraTx> = serde_json::from_str(&response.text().await?)?;

            let history: Vec<History> = json.into_iter().map(Into::into).collect();
            result.push(history)
        }
        Ok(result)
    }

    pub async fn full_scan(&mut self, wollet: &Wollet) -> Result<Option<Update>, Error> {
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
                let result: Vec<Vec<History>> = self.get_scripts_history(&s).await?;
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
        let new_txs = self
            .download_txs(&history_txs_id, &scripts, store, &descriptor)
            .await?;
        let history_txs_heights: HashSet<Height> =
            txid_height.values().filter_map(|e| *e).collect();
        let timestamps = self
            .download_headers(&history_txs_heights, &height_blockhash, store)
            .await?;

        let store_last_unused_external = store
            .cache
            .last_unused_external
            .load(atomic::Ordering::Relaxed);
        let store_last_unused_internal = store
            .cache
            .last_unused_internal
            .load(atomic::Ordering::Relaxed);

        let tip = self.tip().await?;

        let last_unused_changed = store_last_unused_external != last_unused_external
            || store_last_unused_internal != last_unused_internal;

        let changed = !new_txs.txs.is_empty()
            || last_unused_changed
            || !scripts.is_empty()
            || !timestamps.is_empty()
            || store.cache.tip != (tip.height, tip.block_hash());

        if changed {
            tracing::debug!("something changed: !new_txs.txs.is_empty():{} last_unused_changed:{} !scripts.is_empty():{} !timestamps.is_empty():{}", !new_txs.txs.is_empty(), last_unused_changed, !scripts.is_empty(), !timestamps.is_empty() );

            let txid_height_new: Vec<_> = txid_height
                .iter()
                .filter(|(k, v)| match store.cache.heights.get(*k) {
                    Some(e) => e != *v,
                    None => true,
                })
                .map(|(k, v)| (*k, *v))
                .collect();
            let txid_height_delete: Vec<_> = store
                .cache
                .heights
                .keys()
                .filter(|k| txid_height.get(*k).is_none())
                .cloned()
                .collect();

            let update = Update {
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

    async fn download_txs(
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
            let txs_downloaded = self.get_transactions(&txs_to_download).await?;

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
                for tx in self.get_transactions(&txs_to_download).await? {
                    txs.push((tx.txid(), tx));
                }
            }
            Ok(DownloadTxResult { txs, unblinds })
        } else {
            Ok(DownloadTxResult::default())
        }
    }

    async fn download_headers(
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
            for h in self
                .get_headers(&heights_to_download, height_blockhash)
                .await?
            {
                result.push((h.height, h.time))
            }

            tracing::debug!("{} headers_downloaded", heights_to_download.len());
        }

        Ok(result)
    }
}

async fn get_with_retry(url: &str) -> Result<Response, Error> {
    let mut attempt = 0;
    loop {
        let response = reqwest::get(url).await?;
        tracing::debug!(
            "{} status_code:{} body bytes:{:?}",
            &url,
            response.status(),
            response.content_length(),
        );

        if response.status() == 429 {
            if attempt > 6 {
                return Err(Error::Generic("Too many retry".to_string()));
            }
            let secs = 1 << attempt;

            tracing::debug!("waiting {secs}");
            tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
            attempt += 1;
        } else {
            return Ok(response);
        }
    }
}

impl From<EsploraTx> for History {
    fn from(value: EsploraTx) -> Self {
        History {
            txid: value.txid,
            height: value.status.block_height,
            block_hash: Some(value.status.block_hash),
        }
    }
}

#[derive(Deserialize)]
struct EsploraTx {
    txid: elements::Txid,
    status: Status,
}

// TODO some of this fields may be Option in unconfirmed

#[derive(Deserialize)]
struct Status {
    block_height: i32,
    block_hash: BlockHash,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::EsploraWasmClient;
    use elements::{encode::Decodable, BlockHash};

    async fn get_block(base_url: &str, hash: BlockHash) -> elements::Block {
        let url = format!("{}/block/{}/raw", base_url, hash);
        let response = super::get_with_retry(&url).await.unwrap();
        let block =
            elements::Block::consensus_decode(&response.bytes().await.unwrap()[..]).unwrap();
        block
    }

    #[tokio::test]
    async fn esplora_wasm_local() {
        let server = lwk_test_util::setup(true);

        let esplora_url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
        test_esplora_url(&esplora_url).await;
    }

    #[ignore]
    #[tokio::test]
    async fn esplora_wasm_testnet() {
        test_esplora_url("https://blockstream.info/liquidtestnet/api").await;
        test_esplora_url("https://liquid.network/liquidtestnet/api").await;
    }

    async fn test_esplora_url(esplora_url: &str) {
        println!("{}", esplora_url);

        let mut client = EsploraWasmClient::new(esplora_url);
        let header = client.tip().await.unwrap();
        assert!(header.height > 100);

        let headers = client.get_headers(&[0], &HashMap::new()).await.unwrap();
        let genesis_header = &headers[0];
        assert_eq!(genesis_header.height, 0);

        let genesis_block = get_block(esplora_url, genesis_header.block_hash()).await;
        let genesis_tx = &genesis_block.txdata[0];

        let txid = genesis_tx.txid();
        let txs = client.get_transactions(&[txid]).await.unwrap();

        assert_eq!(txs[0].txid(), txid);

        let existing_script = &genesis_tx.output[0].script_pubkey;

        let histories = client
            .get_scripts_history(&[existing_script])
            .await
            .unwrap();
        assert!(!histories.is_empty())
    }
}
