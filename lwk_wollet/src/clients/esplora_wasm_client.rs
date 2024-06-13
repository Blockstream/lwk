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
use elements_miniscript::DescriptorPublicKey;
use reqwest::Response;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    ops::{Index, IndexMut},
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
    waterfall: bool,
}

#[derive(Debug, PartialEq, Eq, Default)]
struct LastUnused {
    internal: u32,
    external: u32,
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

#[derive(Debug, PartialEq, Eq, Default)]
struct Data {
    txid_height: HashMap<Txid, Option<Height>>,
    scripts: HashMap<Script, (Chain, ChildNumber)>,
    last_unused: LastUnused,
    height_blockhash: HashMap<Height, BlockHash>,
    height_timestamp: HashMap<Height, Timestamp>,
}

#[derive(Deserialize)]
struct WaterfallResult {
    txs_seen: HashMap<String, Vec<Vec<History>>>,
    page: u16,
}

impl EsploraWasmClient {
    /// Creates a new esplora client using the given `url` as endpoint.
    ///
    /// If `waterfall` is true, it expects the server support the descriptor endpoint, which avoids several roundtrips
    /// during the scan and for this reason is much faster. To achieve so the "bitcoin descriptor" part is shared with
    /// the server. All of the address are shared with the server anyway even without the waterfall sync, but in
    /// separate calls, and in this case future addresses cannot be derived.
    /// In both cases, the server can see transactions that are involved in the wallet but it knows nothing about the
    /// assets and amount exchanged due to the nature of confidential transactions.
    pub fn new(url: &str, waterfall: bool) -> Self {
        Self {
            base_url: url.to_string(),
            tip_hash_url: format!("{url}/blocks/tip/hash"),
            broadcast_url: format!("{url}/tx"),
            waterfall,
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
        let tx_hex = tx.serialize().to_hex();
        let client = reqwest::Client::new();
        let response = client.post(&self.broadcast_url).body(tx_hex).send().await?;
        let txid = elements::Txid::from_str(&response.text().await?)?;
        Ok(txid)
    }

    async fn get_transaction(&self, txid: Txid) -> Result<elements::Transaction, Error> {
        let tx_url = format!("{}/tx/{}/raw", self.base_url, txid);
        let response = get_with_retry(&tx_url).await?;
        let tx = elements::Transaction::consensus_decode(&response.bytes().await?[..])?;

        Ok(tx)
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

        let Data {
            txid_height,
            scripts,
            last_unused,
            height_blockhash,
            height_timestamp,
        } = if self.waterfall {
            self.get_history_waterfall(&descriptor, store).await?
        } else {
            self.get_history(&descriptor, store).await?
        };

        let tip = self.tip().await?;

        let history_txs_id: HashSet<Txid> = txid_height.keys().cloned().collect();
        let new_txs = self
            .download_txs(&history_txs_id, &scripts, store, &descriptor)
            .await?;
        let history_txs_heights_plus_tip: HashSet<Height> = txid_height
            .values()
            .filter_map(|e| *e)
            .chain(std::iter::once(tip.height))
            .collect();
        let timestamps = self
            .download_headers(
                &history_txs_heights_plus_tip,
                &height_blockhash,
                &height_timestamp,
                store,
            )
            .await?;

        let store_last_unused_external = store
            .cache
            .last_unused_external
            .load(atomic::Ordering::Relaxed);
        let store_last_unused_internal = store
            .cache
            .last_unused_internal
            .load(atomic::Ordering::Relaxed);

        let last_unused_changed = store_last_unused_external != last_unused.external
            || store_last_unused_internal != last_unused.internal;

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

    async fn get_history(
        &mut self,
        descriptor: &WolletDescriptor,
        store: &Store,
    ) -> Result<Data, Error> {
        let mut data = Data::default();
        for descriptor in descriptor.descriptor().clone().into_single_descriptors()? {
            let mut batch_count = 0;
            let chain: Chain = (&descriptor).try_into().unwrap_or(Chain::External);
            loop {
                let batch = store.get_script_batch(batch_count, &descriptor)?;

                let s: Vec<_> = batch.value.iter().map(|e| &e.0).collect();
                let result: Vec<Vec<History>> = self.get_scripts_history(&s).await?;
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

    async fn get_history_waterfall(
        &mut self,
        descriptor: &WolletDescriptor,
        store: &Store,
    ) -> Result<Data, Error> {
        let client = reqwest::Client::new();
        let descriptor_url = format!("{}/v1/waterfall", self.base_url);
        // TODO refuse for elip151
        let desc = descriptor.descriptor().to_string(); // TODO remove unneeded key origin for privacy improvement
        let response = client
            .get(&descriptor_url)
            .query(&[("descriptor", desc)])
            .send()
            .await?;
        let status = response.status().as_u16();
        let body = response.text().await?;

        if status != 200 {
            return Err(Error::Generic(body));
        }
        println!("{body}");
        let waterfall_result: WaterfallResult = serde_json::from_str(&body)?;
        let mut data = Data::default();

        for (desc, chain_history) in waterfall_result.txs_seen.iter() {
            let desc: elements_miniscript::Descriptor<DescriptorPublicKey> = desc.parse()?;
            let chain: Chain = (&desc)
                .try_into()
                .map_err(|_| Error::Generic("Cannot determine chain from desc".into()))?;
            let max = chain_history
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_empty())
                .map(|(i, _)| i as u32)
                .max();
            if let Some(max) = max {
                data.last_unused[chain] = max + 1;
            }
            for (i, script_history) in chain_history.iter().enumerate() {
                // TODO handle paging by asking following pages if there are more than 1000 results
                let child = ChildNumber::from(waterfall_result.page as u32 * 1000 + i as u32);
                let (script, cached) = store.get_or_derive(chain, child, &desc)?;
                if !cached {
                    data.scripts.insert(script, (chain, child));
                }
                for tx_seen in script_history {
                    let height = if tx_seen.height > 0 {
                        Some(tx_seen.height as u32)
                    } else {
                        None
                    };
                    if let Some(height) = height.as_ref() {
                        if let Some(block_hash) = tx_seen.block_hash.as_ref() {
                            data.height_blockhash.insert(*height, *block_hash);
                        }
                        if let Some(ts) = tx_seen.block_timestamp.as_ref() {
                            data.height_timestamp.insert(*height, *ts);
                        }
                    }

                    data.txid_height.insert(tx_seen.txid, height);
                }
            }
        }

        Ok(data)
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

        for txid in txs_to_download {
            let tx = self.get_transaction(txid).await?;

            txs_in_db.insert(txid);

            for (i, output) in tx.output.iter().enumerate() {
                // could be the searched script it's not yet in the store, because created in the current run, thus it's searched also in the `scripts`
                if store.cache.paths.contains_key(&output.script_pubkey)
                    || scripts.contains_key(&output.script_pubkey)
                {
                    let vout = i as u32;
                    let outpoint = OutPoint { txid, vout };

                    match try_unblind(output.clone(), descriptor) {
                            Ok(unblinded) => unblinds.push((outpoint, unblinded)),
                            Err(_) => tracing::info!("{} cannot unblind, ignoring (could be sender messed up with the blinding process)", outpoint),
                        }
                }
            }

            txs.push((txid, tx));
        }

        Ok(DownloadTxResult { txs, unblinds })
    }

    async fn download_headers(
        &self,
        history_txs_heights_plus_tip: &HashSet<Height>,
        height_blockhash: &HashMap<Height, BlockHash>,
        height_timestamp: &HashMap<Height, Timestamp>,
        store: &Store,
    ) -> Result<Vec<(Height, Timestamp)>, Error> {
        let mut result = vec![];
        let heights_in_db: HashSet<Height> = store.cache.timestamps.keys().cloned().collect();
        let heights_in_response: HashSet<Height> = height_timestamp.keys().cloned().collect();
        let heights_in_both: HashSet<Height> =
            heights_in_db.union(&heights_in_response).cloned().collect();

        let heights_to_download: Vec<Height> = history_txs_heights_plus_tip
            .difference(&heights_in_both)
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

        let heights_to_insert = height_timestamp
            .iter()
            .filter(|e| !heights_in_db.contains(e.0))
            .map(|(h, t)| (*h, *t));
        result.extend(heights_to_insert);

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

        // 429 Too many requests
        // 503 Service Temporarily Unavailable
        if response.status() == 429 || response.status() == 503 {
            if attempt > 6 {
                return Err(Error::Generic("Too many retry".to_string()));
            }
            let secs = 1 << attempt;

            tracing::debug!("waiting {secs}");

            async_sleep(secs * 1000).await;
            attempt += 1;
        } else {
            return Ok(response);
        }
    }
}

// based on https://users.rust-lang.org/t/rust-wasm-async-sleeping-for-100-milli-seconds-goes-up-to-1-minute/81177
// TODO remove/handle/justify unwraps
#[cfg(target_arch = "wasm32")]
pub async fn async_sleep(millis: i32) {
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
            .unwrap();
    };
    let p = js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
}
#[cfg(not(target_arch = "wasm32"))]
pub async fn async_sleep(millis: i32) {
    tokio::time::sleep(tokio::time::Duration::from_millis(millis as u64)).await;
}

impl From<EsploraTx> for History {
    fn from(value: EsploraTx) -> Self {
        History {
            txid: value.txid,
            height: value.status.block_height.unwrap_or(-1),
            block_hash: value.status.block_hash,
            block_timestamp: None,
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
    block_height: Option<i32>,
    block_hash: Option<BlockHash>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::EsploraWasmClient;
    use elements::{encode::Decodable, BlockHash};

    async fn get_block(base_url: &str, hash: BlockHash) -> elements::Block {
        let url = format!("{}/block/{}/raw", base_url, hash);
        let response = super::get_with_retry(&url).await.unwrap();
        elements::Block::consensus_decode(&response.bytes().await.unwrap()[..]).unwrap()
    }

    #[ignore = "Should be integration test, but it is testing private function"]
    #[tokio::test]
    async fn esplora_wasm_local() {
        let server = lwk_test_util::setup(true);

        let esplora_url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
        test_esplora_url(&esplora_url).await;
    }

    #[tokio::test]
    async fn sleep_test() {
        // TODO this doesn't last a second when run, is it right?
        super::async_sleep(1).await;
    }

    #[ignore]
    #[tokio::test]
    async fn esplora_wasm_testnet() {
        test_esplora_url("https://blockstream.info/liquidtestnet/api").await;
        test_esplora_url("https://liquid.network/liquidtestnet/api").await;
    }

    async fn test_esplora_url(esplora_url: &str) {
        let mut client = EsploraWasmClient::new(esplora_url, false);
        let header = client.tip().await.unwrap();
        assert!(header.height > 100);

        let headers = client.get_headers(&[0], &HashMap::new()).await.unwrap();
        let genesis_header = &headers[0];
        assert_eq!(genesis_header.height, 0);

        let genesis_block = get_block(esplora_url, genesis_header.block_hash()).await;
        let genesis_tx = &genesis_block.txdata[0];

        let txid = genesis_tx.txid();
        let tx = client.get_transaction(txid).await.unwrap();

        assert_eq!(tx.txid(), txid);

        let existing_script = &genesis_tx.output[0].script_pubkey;

        let histories = client
            .get_scripts_history(&[existing_script])
            .await
            .unwrap();
        assert!(!histories.is_empty())
    }
}
