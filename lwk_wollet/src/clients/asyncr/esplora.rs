//! NOTE This module is temporary, as soon we make the other clients async this will be merged in
//! the standard esplora client of which contain a lot of duplicated code.

use crate::clients::{create_dummy_tx, try_unblind, Capability, History};
use crate::clients::{EsploraClientBuilder, LastUnused};
use crate::BlindingPublicKey;
use crate::{
    clients::Data,
    store::{Height, Store, Timestamp, BATCH_SIZE},
    update::DownloadTxResult,
    wollet::WolletState,
    Chain, ElementsNetwork, Error, Update, Wollet, WolletDescriptor,
};
use age::x25519::Recipient;
use base64::Engine;
use elements::{bitcoin::bip32::ChildNumber, Address, OutPoint};
use elements::{
    encode::Decodable, hashes::hex::FromHex, hex::ToHex, pset::serialize::Serialize, BlockHash,
    Script, Txid,
};
use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use futures::stream::{iter, StreamExt};
use reqwest::Response;
use serde::Deserialize;
use std::sync::atomic::AtomicUsize;
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    str::FromStr,
    sync::atomic,
};

// TODO: Perhaps the waterfalls server's MAX_ADDRESSES could be configurable and return
// the max page size in the response, so we know when we have to request another page
const WATERFALLS_MAX_ADDRESSES: usize = 1_000;

#[derive(Debug)]
/// A blockchain backend implementation based on the
/// [esplora HTTP API](https://github.com/blockstream/esplora/blob/master/API.md)
pub struct EsploraClient {
    client: reqwest::Client,
    base_url: String,
    tip_hash_url: String,
    broadcast_url: String,
    waterfalls: bool,
    pub(crate) utxo_only: bool,
    waterfalls_server_recipient: Option<Recipient>,

    /// Map of a descriptor to its encrypted descriptor.
    /// This is used to avoid encrypting the descriptor field at every requst, which cause
    /// to have a different URL for each request (different salt) which cause http caching to be ineffective.
    /// It's a map because the same client can be used with effectively different descriptor.
    waterfalls_encrypted_descriptors: HashMap<String, String>,

    concurrency: usize,

    /// Avoid encrypting the descriptor field
    pub(crate) waterfalls_avoid_encryption: bool,

    network: ElementsNetwork,

    /// Number of network requests made by this client
    requests: AtomicUsize,
}

impl EsploraClient {
    /// Creates a new esplora client with default options using the given `url` as endpoint.
    ///
    /// To specify different options use the [`EsploraClientBuilder`]
    pub fn new(network: ElementsNetwork, url: &str) -> Self {
        EsploraClientBuilder::new(url, network)
            .build()
            .expect("cannot fail with this configuration")
    }

    pub(crate) async fn last_block_hash(&mut self) -> Result<elements::BlockHash, crate::Error> {
        let response = self.get_with_retry(&self.tip_hash_url).await?;
        Ok(BlockHash::from_str(&response.text().await?)?)
    }

    /// Async version of [`crate::blocking::BlockchainBackend::tip()`]
    pub async fn tip(&mut self) -> Result<elements::BlockHeader, crate::Error> {
        let last_block_hash = self.last_block_hash().await?;

        self.header(last_block_hash).await
    }

    async fn header(&mut self, last_block_hash: BlockHash) -> Result<elements::BlockHeader, Error> {
        let header_url = format!("{}/block/{}/header", self.base_url, last_block_hash);
        let response = self.get_with_retry(&header_url).await?;
        let header_bytes = Vec::<u8>::from_hex(&response.text().await?)?;

        let header = elements::BlockHeader::consensus_decode(&header_bytes[..])?;
        Ok(header)
    }

    /// Async version of [`crate::blocking::BlockchainBackend::broadcast()`]
    pub async fn broadcast(
        &self,
        tx: &elements::Transaction,
    ) -> Result<elements::Txid, crate::Error> {
        // TODO: check that the transaction contains some signatures

        let tx_hex = tx.serialize().to_hex();
        self.requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let response = self
            .client
            .post(&self.broadcast_url)
            .body(tx_hex)
            .send()
            .await?;
        let text = response.text().await?;
        let txid = elements::Txid::from_str(&text).map_err(|e| {
            crate::Error::Generic(format!(
                "Failed to parse response to txid. Response: {text}, Error: {e}"
            ))
        })?;
        Ok(txid)
    }

    pub(crate) async fn get_transaction(&self, txid: Txid) -> Result<elements::Transaction, Error> {
        let tx_url = format!("{}/tx/{}/raw", self.base_url, txid);
        let response = self.get_with_retry(&tx_url).await?;
        let tx = elements::Transaction::consensus_decode(&response.bytes().await?[..])?;

        Ok(tx)
    }

    /// Fetcth concurrently a list of transactions.
    pub async fn get_transactions(
        &self,
        txids: &[Txid],
    ) -> Result<Vec<elements::Transaction>, Error> {
        let stream = iter(txids.iter().cloned())
            .map(|txid| self.get_transaction(txid))
            .buffer_unordered(self.concurrency);

        let results: Vec<Result<elements::Transaction, Error>> = stream.collect().await;
        results.into_iter().collect()
    }

    /// Fetch concurrently a list of block headers
    ///
    /// Optionally pass known blockhash to avoid some network roundtrips if already known.
    pub async fn get_headers(
        &self,
        heights: &[Height],
        height_blockhash: &HashMap<Height, BlockHash>,
    ) -> Result<Vec<elements::BlockHeader>, Error> {
        let stream = iter(heights.iter().cloned())
            .map(|height| async move {
                let block_hash = match height_blockhash.get(&height) {
                    Some(block_hash) => *block_hash,
                    None => {
                        let block_height = format!("{}/block-height/{}", self.base_url, height);
                        let response = self.get_with_retry(&block_height).await?;
                        BlockHash::from_str(&response.text().await?)?
                    }
                };

                let block_header = format!("{}/block/{}/header", self.base_url, block_hash);
                let response = self.get_with_retry(&block_header).await?;
                let header_bytes = Vec::<u8>::from_hex(&response.text().await?)?;

                let header = elements::BlockHeader::consensus_decode(&header_bytes[..])?;

                Ok::<elements::BlockHeader, Error>(header)
            })
            .buffered(self.concurrency);

        let results: Vec<Result<elements::BlockHeader, Error>> = stream.collect().await;
        results.into_iter().collect()
    }

    // examples:
    // https://blockstream.info/liquidtestnet/api/address/tex1qntw9m0j2e93n84x975t47ddhgkzx3x8lhfv2nj/txs
    // https://blockstream.info/liquidtestnet/api/scripthash/b50a2a798d876db54acfa0d8dfdc49154ea8defed37b225ec4c9ec7415358ba3/txs
    /// Get the transactions involved in a list of scripts
    pub async fn get_scripts_history(
        &self,
        scripts: &[&Script],
    ) -> Result<Vec<Vec<History>>, Error> {
        let addresses = scripts
            .iter()
            .filter_map(|script| Address::from_script(script, None, self.network.address_params()))
            .collect::<Vec<_>>();
        if addresses.len() != scripts.len() {
            return Err(Error::Generic(
                "script generated is not a known template".to_owned(),
            ));
        }
        if self.waterfalls {
            self.get_scripts_history_waterfalls(&addresses).await
        } else {
            self.get_scripts_history_esplora(&addresses).await
        }
    }

    async fn get_scripts_history_esplora(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Vec<History>>, Error> {
        let mut result = vec![];
        for address in addresses.iter() {
            let url = format!("{}/address/{}/txs", self.base_url, address);
            // TODO must handle paging -> https://github.com/blockstream/esplora/blob/master/API.md#addresses
            let response = self.get_with_retry(&url).await?;

            // TODO going through string and then json is not as efficient as it could be but we prioritize debugging for now
            let text = response.text().await?;
            let json: Vec<EsploraTx> = match serde_json::from_str(&text) {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("error {e:?} in converting following text:\n{text}");
                    return Err(e.into());
                }
            };
            let history: Vec<History> = json.into_iter().map(Into::into).collect();
            result.push(history)
        }
        Ok(result)
    }

    async fn get_scripts_history_waterfalls(
        &self,
        addresses: &[Address],
    ) -> Result<Vec<Vec<History>>, Error> {
        let mut result = vec![];
        for address_batch in addresses.chunks(50) {
            let url = format!("{}/v2/waterfalls", self.base_url);
            self.requests
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let response = self
                .client
                .get(&url)
                .query(&[(
                    "addresses",
                    address_batch
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                )])
                .send()
                .await?;
            let status = response.status().as_u16();
            let body = response.text().await?;

            if status != 200 {
                return Err(Error::Generic(body));
            }

            let waterfalls_result: WaterfallsResult = serde_json::from_str(&body)?;

            for (_, chain_history) in waterfalls_result.txs_seen.into_iter() {
                result.extend(chain_history);
            }
        }
        Ok(result)
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
    /// To scan beyond the gap limit use [`crate::clients::blocking::BlockchainBackend::full_scan_to_index()`] instead.
    pub async fn full_scan(&mut self, wollet: &Wollet) -> Result<Option<Update>, Error> {
        self.full_scan_to_index(wollet, 0).await
    }

    /// Scan the blockchain for the scripts generated by a watch-only wallet up to a specified derivation index
    ///
    /// While [`Self::full_scan()`] stops after finding 20 consecutive unused addresses (the gap limit),
    /// this method will scan at least up to the given derivation index. This is useful to prevent
    /// missing funds in cases where outputs exist beyond the gap limit.
    ///
    /// Will scan both external and internal address chains up to the given index for maximum safety,
    /// even though internal addresses may not need such deep scanning.
    ///
    /// If transactions are found beyond the gap limit during this scan, subsequent calls to
    /// [`Self::full_scan()`] will automatically scan up to the highest used index, preventing any
    /// previously-found transactions from being missed.
    ///
    /// See [`crate::blocking::BlockchainBackend::full_scan_to_index()`] for a blocking version of this method.
    pub async fn full_scan_to_index(
        &mut self,
        wollet: &Wollet,
        index: u32,
    ) -> Result<Option<Update>, Error> {
        let descriptor = wollet.wollet_descriptor();
        let store = &wollet.store;

        let Data {
            txid_height,
            scripts,
            last_unused,
            height_blockhash,
            height_timestamp,
            tip,
            unspent,
        } = if self.waterfalls {
            match self
                .get_history_waterfalls(&descriptor, wollet, index)
                .await
            {
                Ok(d) => d,
                Err(Error::UsingWaterfallsWithElip151) => {
                    self.get_history(&descriptor, store, index, wollet.last_unused())
                        .await?
                }
                Err(e) => return Err(e),
            }
        } else {
            self.get_history(&descriptor, store, index, wollet.last_unused())
                .await?
        };

        let tip = if let Some(tip) = tip {
            self.header(tip).await?
        } else {
            self.tip().await?
        };

        let history_txs_id: HashSet<Txid> = txid_height.keys().cloned().collect();
        let mut new_txs = self
            .download_txs(&history_txs_id, &scripts, store, &descriptor)
            .await?;

        if self.utxo_only {
            let tx = create_dummy_tx(&unspent, &new_txs);
            new_txs.txs.push((tx.txid(), tx));
        }

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
            log::debug!("something changed: !new_txs.txs.is_empty():{} last_unused_changed:{} !scripts.is_empty():{} !timestamps.is_empty():{}", !new_txs.txs.is_empty(), last_unused_changed, !scripts.is_empty(), !timestamps.is_empty() );

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
                .filter(|k| !txid_height.contains_key(*k))
                .cloned()
                .collect();
            let wollet_status = wollet.status();

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

    /// Async version of [`crate::blocking::BlockchainBackend::get_history()`]
    async fn get_history(
        &mut self,
        descriptor: &WolletDescriptor,
        store: &Store,
        index: u32,
        last_unused: LastUnused,
    ) -> Result<Data, Error> {
        let mut data = Data::default();

        for descriptor in descriptor.as_single_descriptors()? {
            let mut batch_count = 0;
            let chain: Chain = (&descriptor).try_into().unwrap_or(Chain::External);
            let index = index.max(last_unused[chain]);
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
            }
        }
        Ok(data)
    }

    /// Returns the waterfall server recipient key using a cached value or by asking the server its key
    pub(crate) async fn waterfalls_server_recipient(&mut self) -> Result<Recipient, Error> {
        match self.waterfalls_server_recipient.as_ref() {
            Some(r) => Ok(r.clone()),
            None => {
                let url = format!("{}/v1/server_recipient", self.base_url);
                self.requests
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let response = self.client.get(&url).send().await?;
                let status = response.status().as_u16();
                let body = response.text().await?;
                if status != 200 {
                    return Err(Error::Generic(body));
                }
                let rec = Recipient::from_str(&body).map_err(|_| Error::CannotParseRecipientKey)?;
                self.waterfalls_server_recipient = Some(rec.clone());
                Ok(rec)
            }
        }
    }

    pub(crate) async fn get_history_waterfalls<S: WolletState>(
        &mut self,
        descriptor: &WolletDescriptor,
        store: &S,
        to_index: u32,
    ) -> Result<Data, Error> {
        let descriptor_url = format!("{}/v2/waterfalls", self.base_url);
        if descriptor.is_elip151() {
            return Err(Error::UsingWaterfallsWithElip151);
        }
        let desc = descriptor.bitcoin_descriptor_without_key_origin();
        let desc = if self.waterfalls_avoid_encryption {
            desc
        } else {
            match self.waterfalls_encrypted_descriptors.get(&desc) {
                Some(encrypted_descriptor) => encrypted_descriptor.clone(),
                None => {
                    let recipient = self.waterfalls_server_recipient().await?;
                    let encrypted_descriptor = encrypt(&desc, recipient)?;
                    self.waterfalls_encrypted_descriptors
                        .insert(desc, encrypted_descriptor.clone());
                    encrypted_descriptor
                }
            }
        };

        let mut page = 0;
        let mut data = Data::default();

        loop {
            self.requests
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Log the full URL including query parameters
            log::debug!(
                "Requesting URL: {}?descriptor={}&page={}&to_index={}&utxo_only={}",
                descriptor_url,
                url_encode_descriptor(&desc),
                page,
                to_index,
                self.utxo_only
            );

            let response = self
                .client
                .get(&descriptor_url)
                .query(&[("descriptor", desc.clone())])
                .query(&[("page", page.to_string())])
                .query(&[("to_index", to_index.to_string())])
                .query(&[("utxo_only", self.utxo_only.to_string())])
                .send()
                .await?;
            let status = response.status().as_u16();
            let body = response.text().await?;

            if status != 200 {
                return Err(Error::Generic(body));
            }

            let waterfalls_result: WaterfallsResult = serde_json::from_str(&body)?;

            if self.utxo_only {
                let unspent = waterfalls_result
                    .txs_seen
                    .values()
                    .flatten()
                    .flatten()
                    .map(|h| OutPoint::new(h.txid, (h.v - 1) as u32)); // TODO
                data.unspent.extend(unspent);
            }

            for (desc, chain_history) in waterfalls_result.txs_seen.iter() {
                let desc: elements_miniscript::Descriptor<DescriptorPublicKey> = desc.parse()?;

                // NOTE: in case of descriptors without path ending, the Chain will be wrong, and the
                // `Data::scripts` will be wrong too, this doesn't seem to impact the correctness of the scan.
                let chain: Chain = (&desc).try_into().unwrap_or(Chain::External);

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
                    let child = ChildNumber::from(
                        waterfalls_result.page as u32 * WATERFALLS_MAX_ADDRESSES as u32 + i as u32,
                    );
                    let ct_desc = ConfidentialDescriptor {
                        key: descriptor.as_ref().key.clone(),
                        descriptor: desc.clone(),
                    };
                    let (script, blinding_pubkey, cached) =
                        store.get_or_derive(chain, child, &ct_desc)?;
                    if !cached {
                        data.scripts.insert(script, (chain, child, blinding_pubkey));
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
            data.tip = waterfalls_result.tip;
            page = waterfalls_result.page + 1;

            let total = waterfalls_result
                .txs_seen
                .values()
                .map(|chain_history| chain_history.len())
                .max()
                .unwrap_or(0);

            if total < WATERFALLS_MAX_ADDRESSES {
                break;
            }
        }

        Ok(data)
    }

    /// Avoid encrypting the descriptor when calling the waterfalls endpoint.
    pub fn avoid_encryption(&mut self) {
        self.waterfalls_avoid_encryption = true;
    }

    /// Set the waterfalls server recipient key. This is used to encrypt the descriptor when calling the waterfalls endpoint.
    pub fn set_waterfalls_server_recipient(&mut self, recipient: Recipient) {
        self.waterfalls_server_recipient = Some(recipient);
    }

    async fn download_txs(
        &self,
        history_txs_id: &HashSet<Txid>,
        scripts: &HashMap<Script, (Chain, ChildNumber, BlindingPublicKey)>,
        store: &Store,
        descriptor: &WolletDescriptor,
    ) -> Result<DownloadTxResult, Error> {
        let mut txs = vec![];
        let mut unblinds = vec![];

        let mut txs_in_db = store.cache.all_txs.keys().cloned().collect();
        let txs_to_download: Vec<Txid> = history_txs_id.difference(&txs_in_db).cloned().collect();

        let mut stream = iter(txs_to_download.iter().cloned())
            .map(|txid| async move {
                let tx = self.get_transaction(txid).await?;
                Ok::<(Txid, elements::Transaction), Error>((txid, tx))
            })
            .buffer_unordered(self.concurrency);

        while let Some(result) = stream.next().await {
            match result {
                Ok((txid, tx)) => {
                    txs_in_db.insert(txid);

                    for (i, output) in tx.output.iter().enumerate() {
                        // could be the searched script it's not yet in the store, because created in the current run, thus it's searched also in the `scripts`
                        if store.cache.paths.contains_key(&output.script_pubkey)
                            || scripts.contains_key(&output.script_pubkey)
                        {
                            let vout = i as u32;
                            let outpoint = OutPoint { txid, vout };

                            match try_unblind(output, descriptor) {
                                    Ok(unblinded) => unblinds.push((outpoint, unblinded)),
                                    Err(_) => log::info!("{} cannot unblind, ignoring (could be sender messed up with the blinding process)", outpoint),
                                }
                        }
                    }

                    txs.push((txid, tx));
                }
                Err(e) => return Err(e),
            }
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

            log::debug!("{} headers_downloaded", heights_to_download.len());
        }

        let heights_to_insert = height_timestamp
            .iter()
            .filter(|e| !heights_in_db.contains(e.0))
            .map(|(h, t)| (*h, *t));
        result.extend(heights_to_insert);

        Ok(result)
    }

    #[allow(unused)]
    pub(crate) fn capabilities(&self) -> HashSet<Capability> {
        if self.waterfalls {
            vec![Capability::Waterfalls].into_iter().collect()
        } else {
            HashSet::new()
        }
    }

    /// Return the number of network requests made by this client.
    pub fn requests(&self) -> usize {
        self.requests.load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn get_with_retry(&self, url: &str) -> Result<Response, Error> {
        let mut attempt = 0;
        loop {
            self.requests
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let response = self.client.get(url).send().await?;

            let level = if response.status() == 200 {
                log::Level::Trace
            } else {
                log::Level::Info
            };
            log::log!(
                level,
                "{} status_code:{} - body bytes:{:?}",
                &url,
                response.status(),
                response.content_length(),
            );

            // 429 Too many requests
            // 503 Service Temporarily Unavailable
            if response.status() == 429 || response.status() == 503 {
                if attempt > 6 {
                    log::warn!("{url} tried 6 times, failing");
                    return Err(Error::Generic("Too many retry".to_string()));
                }
                let secs = 1 << attempt;

                log::debug!("{url} waiting {secs}");

                async_sleep(secs * 1000).await;
                attempt += 1;
            } else {
                return Ok(response);
            }
        }
    }
}

impl EsploraClientBuilder {
    /// Consume the builder and build a new [`EsploraClient`]
    pub fn build(self) -> Result<EsploraClient, Error> {
        if !self.waterfalls && self.utxo_only {
            return Err(Error::Generic(
                "UTXO only can be used only with waterfalls".to_string(),
            ));
        }
        let headers = (&self.headers).try_into().expect("Expected valid headers");
        #[cfg(target_arch = "wasm32")]
        let builder = reqwest::Client::builder().default_headers(headers);
        #[cfg(not(target_arch = "wasm32"))]
        let mut builder = reqwest::Client::builder().default_headers(headers);
        // See https://github.com/seanmonstar/reqwest/issues/1135
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(std::time::Duration::from_secs(timeout as u64));
        }
        let client = builder.build().expect("Failed to create client"); // TODO: handle error but note that this is equivalent to the new() which panics
        Ok(EsploraClient {
            client,
            base_url: self.base_url.clone(),
            tip_hash_url: format!("{}/blocks/tip/hash", self.base_url),
            broadcast_url: format!("{}/tx", self.base_url),
            waterfalls: self.waterfalls,
            utxo_only: self.utxo_only,
            waterfalls_server_recipient: None,
            waterfalls_avoid_encryption: false,
            network: self.network,
            concurrency: self.concurrency.unwrap_or(1),
            requests: AtomicUsize::new(0),
            waterfalls_encrypted_descriptors: HashMap::new(),
        })
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
/// Sleep asynchronously for the given number of milliseconds on non-WASM targets.
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
            v: 0,
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

/// The result of a "waterfalls" descriptor endpoint call
#[derive(Deserialize)]
struct WaterfallsResult {
    pub txs_seen: HashMap<String, Vec<Vec<History>>>,
    pub page: u16,
    pub tip: Option<BlockHash>,
}

/// Encrypt a plaintext using a recipient key
///
/// This can be used to encrypt a descriptor to share with a "waterfalls" server
fn encrypt(plaintext: &str, recipient: Recipient) -> Result<String, Error> {
    let recipients = [recipient];
    let encryptor =
        age::Encryptor::with_recipients(recipients.iter().map(|e| e as &dyn age::Recipient))
            .expect("we provided a recipient");

    let mut encrypted = vec![];
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|_| Error::CannotEncrypt)?;
    writer.write_all(plaintext.as_ref())?;
    writer.finish()?;
    let result = base64::prelude::BASE64_STANDARD_NO_PAD.encode(encrypted);
    Ok(result)
}

/// Simple URL encoding for common characters found in Bitcoin descriptors
fn url_encode_descriptor(desc: &str) -> String {
    desc.chars()
        .map(|c| match c {
            '(' => "%28".to_string(),
            ')' => "%29".to_string(),
            '[' => "%5B".to_string(),
            ']' => "%5D".to_string(),
            '{' => "%7B".to_string(),
            '}' => "%7D".to_string(),
            '/' => "%2F".to_string(),
            '*' => "%2A".to_string(),
            ',' => "%2C".to_string(),
            '%' => "%25".to_string(),
            '&' => "%26".to_string(),
            '#' => "%23".to_string(),
            '+' => "%2B".to_string(),
            ' ' => "%20".to_string(),
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use crate::{clients::asyncr::async_sleep, ElementsNetwork};

    use super::{url_encode_descriptor, EsploraClient};
    use elements::{encode::Decodable, BlockHash};

    async fn get_block(base_url: &str, hash: BlockHash) -> elements::Block {
        let url = format!("{}/block/{}/raw", base_url, hash);
        let client = EsploraClient::new(ElementsNetwork::Liquid, base_url);
        let response = client.get_with_retry(&url).await.unwrap();
        elements::Block::consensus_decode(&response.bytes().await.unwrap()[..]).unwrap()
    }

    #[ignore = "Should be integration test, but it is testing private function"]
    #[tokio::test]
    async fn esplora_wasm_local() {
        let server = lwk_test_util::setup_with_esplora();

        let esplora_url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
        test_esplora_url(&esplora_url).await;
    }

    #[tokio::test]
    async fn sleep_test() {
        // TODO this doesn't last a second when run, is it right?
        async_sleep(1).await;
    }

    #[test]
    fn test_url_encode_descriptor() {
        // Test basic descriptor encoding
        let desc = "ct(slip77(key),elwpkh(xpub/*))";
        let encoded = url_encode_descriptor(desc);
        assert_eq!(encoded, "ct%28slip77%28key%29%2Celwpkh%28xpub%2F%2A%29%29");

        // Test descriptor with brackets
        let desc_with_brackets = "ct(key,elwpkh([fingerprint/path]xpub/*))";
        let encoded_brackets = url_encode_descriptor(desc_with_brackets);
        assert_eq!(
            encoded_brackets,
            "ct%28key%2Celwpkh%28%5Bfingerprint%2Fpath%5Dxpub%2F%2A%29%29"
        );

        // Test descriptor with curly braces
        let desc_with_braces = "ct({key},elwpkh(xpub/*))";
        let encoded_braces = url_encode_descriptor(desc_with_braces);
        assert_eq!(encoded_braces, "ct%28%7Bkey%7D%2Celwpkh%28xpub%2F%2A%29%29");

        // Test normal characters (should remain unchanged)
        let normal = "ctabc123def";
        assert_eq!(url_encode_descriptor(normal), "ctabc123def");
    }

    #[ignore]
    #[tokio::test]
    async fn esplora_wasm_testnet() {
        test_esplora_url("https://blockstream.info/liquidtestnet/api").await;
        test_esplora_url("https://liquid.network/liquidtestnet/api").await;
        test_esplora_url("https://waterfalls.liquidwebwallet.org/liquidtestnet/api").await;

        test_esplora_url("https://blockstream.info/liquid/api").await;
        test_esplora_url("https://liquid.network/liquid/api").await;
        test_esplora_url("https://waterfalls.liquidwebwallet.org/liquid/api").await;
    }

    async fn test_esplora_url(esplora_url: &str) {
        let (network, txid) = if esplora_url.contains("liquidtestnet") {
            (
                ElementsNetwork::LiquidTestnet,
                "0471d2f856b3fdbc4397af272bee1660b77aaf9a4aeb86fdd96110ce00f2b158",
            )
        } else if esplora_url.contains("liquid") {
            (
                ElementsNetwork::Liquid,
                "efb331fb5051a3b638ddbe719482dcb5232096448bd0a73550408c84bc2269ea",
            )
        } else {
            (
                ElementsNetwork::default_regtest(),
                "0000000000000000000000000000000000000000000000000000000000000000",
            )
        };
        let is_waterfalls = esplora_url.contains("waterfalls");
        let mut client = EsploraClient::new(network, esplora_url);
        let header = client.tip().await.unwrap();
        assert!(header.height > 100);

        let headers = client.get_headers(&[0], &HashMap::new()).await.unwrap();
        let genesis_header = &headers[0];
        assert_eq!(genesis_header.height, 0);

        if !is_waterfalls {
            // waterfalls doesn't have the block endpoint
            let _ = get_block(esplora_url, genesis_header.block_hash()).await;
        }
        let txid = elements::Txid::from_str(txid).unwrap();

        let tx = client.get_transaction(txid).await.unwrap();
        assert_eq!(tx.txid(), txid);

        // Test get_transactions method with the same txid
        let txs_batch = client.get_transactions(&[txid]).await.unwrap();
        assert_eq!(txs_batch.len(), 1);
        assert_eq!(txs_batch[0].txid(), txid);

        let existing_script =
            elements::Script::from_str("001414fe45f2c2a2b7c00d0940d694a3b6af6c9bf165").unwrap();

        let histories = client
            .get_scripts_history(&[&existing_script])
            .await
            .unwrap();
        assert!(!histories.is_empty())
    }

    #[test]
    fn test_esplora_client_builder_error() {
        let client = crate::asyncr::EsploraClientBuilder::new("", ElementsNetwork::Liquid)
            .waterfalls(false)
            .utxo_only(true)
            .build();
        assert!(client.is_err());
    }
}
