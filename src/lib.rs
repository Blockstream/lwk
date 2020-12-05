#[macro_use]
extern crate lazy_static;

pub mod be;
pub mod error;
pub mod headers;
pub mod interface;
pub mod model;
pub mod network;
pub mod scripts;
mod store;

pub use network::*;
use wally::asset_unblind;

use std::time::Duration;

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

use crate::error::Error;
use crate::model::{
    CreateTransactionOpt, GetTransactionsOpt, TransactionDetails, TXO,
};

use crate::be::*;
use crate::headers::bitcoin::HeadersChain;
use crate::headers::liquid::Verifier;
use crate::headers::ChainOrVerifier;
use crate::interface::{ElectrumUrl, WalletCtx};
use crate::model::*;
pub use crate::network::Network;
use crate::store::{Indexes, Store, BATCH_SIZE};
pub use crate::{ElementsNetwork, NetworkId};

use log::{debug, info, trace, warn};

use bitcoin::blockdata::constants::DIFFCHANGE_INTERVAL;
use bitcoin::secp256k1;
use bitcoin::util::bip32::DerivationPath;
use bitcoin::{BlockHash, Script, Txid};

use elements::confidential::{self, Asset, Nonce};
use elements::slip77::MasterBlindingKey;

use electrum_client::GetHistoryRes;
use electrum_client::{Client, ElectrumApi};

use rand::seq::SliceRandom;
use rand::thread_rng;

struct Syncer {
    pub store: Store,
    pub master_blinding: Option<MasterBlindingKey>,
    pub network: Network,
}

struct Tipper {
    pub store: Store,
    pub network: Network,
}

struct Headers {
    pub store: Store,
    pub checker: ChainOrVerifier,
}

struct Closer {
    pub senders: Vec<Sender<()>>,
    pub handles: Vec<JoinHandle<()>>,
}

impl Closer {
    pub fn close(&mut self) -> Result<(), Error> {
        while let Some(sender) = self.senders.pop() {
            sender.send(())?;
        }
        while let Some(handle) = self.handles.pop() {
            handle
                .join()
                .expect("Couldn't join on the associated thread");
        }
        Ok(())
    }
}

fn determine_electrum_url(
    url: &Option<String>,
    tls: Option<bool>,
    validate_domain: Option<bool>,
) -> Result<ElectrumUrl, Error> {
    let url = url
        .as_ref()
        .ok_or_else(|| Error::Generic("network url is missing".into()))?;
    if url == "" {
        return Err(Error::Generic("network url is empty".into()));
    }

    if tls.unwrap_or(false) {
        Ok(ElectrumUrl::Tls(
            url.into(),
            validate_domain.unwrap_or(false),
        ))
    } else {
        Ok(ElectrumUrl::Plaintext(url.into()))
    }
}

pub fn determine_electrum_url_from_net(network: &Network) -> Result<ElectrumUrl, Error> {
    determine_electrum_url(&network.electrum_url, network.tls, network.validate_domain)
}

fn try_get_fee_estimates(client: &Client) -> Result<Vec<FeeEstimate>, Error> {
    let relay_fee = (client.relay_fee()? * 100_000_000.0) as u64;
    let blocks: Vec<usize> = (1..25).collect();
    // max is covering a rounding errors in production electrs which sometimes cause a fee
    // estimates lower than relay fee
    let mut estimates: Vec<FeeEstimate> = client
        .batch_estimate_fee(blocks)?
        .iter()
        .map(|e| FeeEstimate(relay_fee.max((*e * 100_000_000.0) as u64)))
        .collect();
    estimates.insert(0, FeeEstimate(relay_fee));
    Ok(estimates)
}

impl Tipper {
    pub fn tip(&self, client: &Client) -> Result<u32, Error> {
        let header = client.block_headers_subscribe_raw()?;
        let height = header.height as u32;
        let tip_height = self.store.read()?.cache.tip.0;
        if height != tip_height {
            let hash = BEBlockHeader::deserialize(&header.header, self.network.id())?.block_hash();
            info!("saving in store new tip {:?}", (height, hash));
            self.store.write()?.cache.tip = (height, hash);
        }
        Ok(height)
    }
}

impl Headers {
    pub fn ask(&mut self, chunk_size: usize, client: &Client) -> Result<usize, Error> {
        if let ChainOrVerifier::Chain(chain) = &mut self.checker {
            info!(
                "asking headers, current height:{} chunk_size:{} ",
                chain.height(),
                chunk_size
            );
            let headers = client
                .block_headers(chain.height() as usize + 1, chunk_size)?
                .headers;
            let len = headers.len();
            chain.push(headers)?;
            Ok(len)
        } else {
            // Liquid doesn't need to download the header's chain
            Ok(0)
        }
    }

    pub fn get_proofs(&mut self, client: &Client) -> Result<usize, Error> {
        let store_read = self.store.read()?;
        let needs_proof: Vec<(Txid, u32)> = self
            .store
            .read()?
            .cache
            .heights
            .iter()
            .filter(|(_, opt)| opt.is_some())
            .map(|(t, h)| (t, h.unwrap()))
            .filter(|(t, _)| store_read.cache.txs_verif.get(*t).is_none())
            .map(|(t, h)| (t.clone(), h))
            .collect();
        drop(store_read);

        let mut txs_verified = HashMap::new();
        for (txid, height) in needs_proof {
            let proof = client.transaction_get_merkle(&txid, height as usize)?;
            let verified = match &self.checker {
                ChainOrVerifier::Chain(chain) => {
                    chain.verify_tx_proof(&txid, height, proof).is_ok()
                }
                ChainOrVerifier::Verifier(verifier) => {
                    if let Some(BEBlockHeader::Elements(header)) =
                        self.store.read()?.cache.headers.get(&height)
                    {
                        verifier.verify_tx_proof(&txid, proof, &header).is_ok()
                    } else {
                        false
                    }
                }
            };
            if verified {
                info!("proof for {} verified!", txid);
                txs_verified.insert(txid, SPVVerifyResult::Verified);
            } else {
                warn!("proof for {} not verified!", txid);
                txs_verified.insert(txid, SPVVerifyResult::NotVerified);
            }
        }
        let proofs_done = txs_verified.len();
        self.store.write()?.cache.txs_verif.extend(txs_verified);
        Ok(proofs_done)
    }

    pub fn remove(&mut self, headers: u32) -> Result<(), Error> {
        if let ChainOrVerifier::Chain(chain) = &mut self.checker {
            chain.remove(headers)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct DownloadTxResult {
    txs: Vec<(Txid, BETransaction)>,
    unblinds: Vec<(elements::OutPoint, Unblinded)>,
}

impl Syncer {
    pub fn sync(&self, client: &Client) -> Result<bool, Error> {
        debug!("start sync");
        let start = Instant::now();

        let mut history_txs_id = HashSet::new();
        let mut heights_set = HashSet::new();
        let mut txid_height = HashMap::new();
        let mut scripts = HashMap::new();

        let mut last_used = Indexes::default();
        let mut wallet_chains = vec![0, 1];
        wallet_chains.shuffle(&mut thread_rng());
        for i in wallet_chains {
            let mut batch_count = 0;
            loop {
                let batch = self.store.read()?.get_script_batch(i, batch_count)?;
                let result: Vec<Vec<GetHistoryRes>> =
                    client.batch_script_get_history(batch.value.iter().map(|e| &e.0))?;
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
                trace!("{}/batch({}) {:?}", i, batch_count, flattened.len());

                if flattened.is_empty() {
                    break;
                }

                for el in flattened {
                    // el.height = -1 means unconfirmed with unconfirmed parents
                    // el.height =  0 means unconfirmed with confirmed parents
                    // but we threat those tx the same
                    let height = el.height.max(0);
                    heights_set.insert(height as u32);
                    if height == 0 {
                        txid_height.insert(el.tx_hash, None);
                    } else {
                        txid_height.insert(el.tx_hash, Some(height as u32));
                    }

                    history_txs_id.insert(el.tx_hash);
                }

                batch_count += 1;
            }
        }

        let new_txs = self.download_txs(&history_txs_id, &scripts, &client)?;
        let headers = self.download_headers(&heights_set, &client)?;

        let store_indexes = self.store.read()?.cache.indexes.clone();

        let changed = if !new_txs.txs.is_empty()
            || !headers.is_empty()
            || store_indexes != last_used
            || !scripts.is_empty()
        {
            info!(
                "There are changes in the store new_txs:{:?} headers:{:?} txid_height:{:?}",
                new_txs.txs.iter().map(|tx| tx.0).collect::<Vec<Txid>>(),
                headers,
                txid_height
            );
            let mut store_write = self.store.write()?;
            store_write.cache.indexes = last_used;
            store_write.cache.all_txs.extend(new_txs.txs.into_iter());
            store_write.cache.unblinded.extend(new_txs.unblinds);
            store_write.cache.headers.extend(headers);

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
        trace!(
            "changes:{} elapsed {}",
            changed,
            start.elapsed().as_millis()
        );

        Ok(changed)
    }

    fn download_headers(
        &self,
        heights_set: &HashSet<u32>,
        client: &Client,
    ) -> Result<Vec<(u32, BEBlockHeader)>, Error> {
        let mut result = vec![];
        let mut heights_in_db: HashSet<u32> = self
            .store
            .read()?
            .cache
            .heights
            .iter()
            .filter_map(|(_, h)| *h)
            .collect();
        heights_in_db.insert(0);
        let heights_to_download: Vec<u32> =
            heights_set.difference(&heights_in_db).cloned().collect();
        if !heights_to_download.is_empty() {
            let headers_bytes_downloaded =
                client.batch_block_header_raw(heights_to_download.clone())?;
            let mut headers_downloaded: Vec<BEBlockHeader> = vec![];
            for vec in headers_bytes_downloaded {
                headers_downloaded.push(BEBlockHeader::deserialize(&vec, self.network.id())?);
            }
            info!("headers_downloaded {:?}", &headers_downloaded);
            for (header, height) in headers_downloaded
                .into_iter()
                .zip(heights_to_download.into_iter())
            {
                result.push((height, header));
            }
        }

        Ok(result)
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
            let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_to_download)?;
            let mut txs_downloaded: Vec<BETransaction> = vec![];
            for vec in txs_bytes_downloaded {
                let tx = BETransaction::deserialize(&vec, self.network.id())?;
                txs_downloaded.push(tx);
            }
            info!("txs_downloaded {:?}", txs_downloaded.len());
            let mut previous_txs_to_download = HashSet::new();
            for mut tx in txs_downloaded.into_iter() {
                let txid = tx.txid();
                txs_in_db.insert(txid);

                if let BETransaction::Elements(tx) = &tx {
                    info!("compute OutPoint Unblinded");
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
                            let outpoint = elements::OutPoint {
                                txid: tx.txid(),
                                vout,
                            };

                            match self.try_unblind(outpoint, output.clone()) {
                                Ok(unblinded) => unblinds.push((outpoint, unblinded)),
                                Err(_) => info!("{} cannot unblind, ignoring (could be sender messed up with the blinding process)", outpoint),
                            }
                        }
                    }
                } else {
                    // download all previous output only for bitcoin (to calculate fee of incoming tx)
                    for previous_txid in tx.previous_output_txids() {
                        previous_txs_to_download.insert(previous_txid);
                    }
                }
                tx.strip_witness();
                txs.push((txid, tx));
            }

            let txs_to_download: Vec<&Txid> =
                previous_txs_to_download.difference(&txs_in_db).collect();
            if !txs_to_download.is_empty() {
                let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_to_download)?;
                for vec in txs_bytes_downloaded {
                    let mut tx = BETransaction::deserialize(&vec, self.network.id())?;
                    tx.strip_witness();
                    txs.push((tx.txid(), tx));
                }
            }
            Ok(DownloadTxResult { txs, unblinds })
        } else {
            Ok(DownloadTxResult::default())
        }
    }

    pub fn try_unblind(
        &self,
        outpoint: elements::OutPoint,
        output: elements::TxOut,
    ) -> Result<Unblinded, Error> {
        match (output.asset, output.value, output.nonce) {
            (
                Asset::Confidential(_, _),
                confidential::Value::Confidential(_, _),
                Nonce::Confidential(_, _),
            ) => {
                let script = output.script_pubkey.clone();
                let blinding_key = self
                    .master_blinding
                    .as_ref()
                    .unwrap()
                    .derive_blinding_key(&script);
                let rangeproof = output.witness.rangeproof.clone();
                let value_commitment = elements::encode::serialize(&output.value);
                let asset_commitment = elements::encode::serialize(&output.asset);
                let nonce_commitment = elements::encode::serialize(&output.nonce);
                info!(
                    "commitments len {} {} {}",
                    value_commitment.len(),
                    asset_commitment.len(),
                    nonce_commitment.len()
                );
                let sender_pk = secp256k1::PublicKey::from_slice(&nonce_commitment).unwrap();

                let (asset, abf, vbf, value) = asset_unblind(
                    sender_pk,
                    blinding_key,
                    rangeproof,
                    value_commitment,
                    script,
                    asset_commitment,
                )?;

                info!(
                    "Unblinded outpoint:{} asset:{} value:{}",
                    outpoint,
                    hex::encode(&asset),
                    value
                );

                let unblinded = Unblinded {
                    asset,
                    value,
                    abf,
                    vbf,
                };
                Ok(unblinded)
            }
            _ => Err(Error::Generic(
                "received unconfidential or null asset/value/nonce".into(),
            )),
        }
    }
}

fn wait_or_close(r: &Receiver<()>, interval: u32) -> bool {
    for _ in 0..(interval * 2) {
        thread::sleep(Duration::from_millis(500));
        if r.try_recv().is_ok() {
            return true;
        }
    }
    false
}

pub struct ElectrumWallet {
    pub data_root: String,
    pub network: Network,
    pub url: ElectrumUrl,
    pub wallet: Option<WalletCtx>,
    closer: Closer,
}

impl ElectrumWallet {
    // when to pass the mnemonic?
    //pub fn start(&mut self) -> Result<(), Error> {
    pub fn start(
        network: Network,
        data_root: &str,
        url: ElectrumUrl,
        mnemonic: &str,
    ) -> Result<Self, Error> {
        let sync_interval = network.sync_interval.unwrap_or(7);

        let wallet = WalletCtx::from_mnemonic(mnemonic, &data_root, network.clone())?;

        let mut closer = Closer {
            senders: vec![],
            handles: vec![],
        };

        let mut tip_height = wallet.store.read()?.cache.tip.0;

        info!("building client");
        if let Ok(fee_client) = url.build_client() {
            info!("building built end");
            let fee_store = wallet.store.clone();
            thread::spawn(move || {
                match try_get_fee_estimates(&fee_client) {
                    Ok(fee_estimates) => {
                        fee_store.write().unwrap().cache.fee_estimates = fee_estimates
                    }
                    Err(e) => warn!("can't update fee estimates {:?}", e),
                };
            });
        }

        if network.spv_enabled.unwrap_or(false) {
            let checker = match network.id() {
                NetworkId::Bitcoin(network) => {
                    let mut path: PathBuf = data_root.into();
                    path.push(format!("headers_chain_{}", network));
                    ChainOrVerifier::Chain(HeadersChain::new(path, network)?)
                }
                NetworkId::Elements(network) => {
                    let verifier = Verifier::new(network);
                    ChainOrVerifier::Verifier(verifier)
                }
            };

            let mut headers = Headers {
                store: wallet.store.clone(),
                checker,
            };

            let headers_url = url.clone();
            let (close_headers, r) = channel();
            closer.senders.push(close_headers);
            let mut chunk_size = DIFFCHANGE_INTERVAL as usize;
            let headers_handle = thread::spawn(move || {
                info!("starting headers thread");

                'outer: loop {
                    if wait_or_close(&r, sync_interval) {
                        info!("closing headers thread");
                        break;
                    }

                    if let Ok(client) = headers_url.build_client() {
                        loop {
                            if r.try_recv().is_ok() {
                                info!("closing headers thread");
                                break 'outer;
                            }
                            match headers.ask(chunk_size, &client) {
                                Ok(headers_found) => {
                                    if headers_found == 0 {
                                        chunk_size = 1
                                    } else {
                                        info!("headers found: {}", headers_found);
                                    }
                                }
                                Err(Error::InvalidHeaders) => {
                                    // this should handle reorgs and also broke IO writes update
                                    if headers.remove(144).is_err() {
                                        break;
                                    }
                                }
                                Err(e) => {
                                    // usual error is because I reached the tip, trying asking half
                                    //TODO this is due to an esplora electrs bug, according to spec it should
                                    // just return available headers, remove when fix is deployed and change previous
                                    // break condition to headers_found < chunk_size
                                    info!("error while asking headers {}", e);
                                    if chunk_size > 1 {
                                        chunk_size /= 2
                                    } else {
                                        break;
                                    }
                                }
                            }
                            if chunk_size == 1 {
                                break;
                            }
                        }

                        match headers.get_proofs(&client) {
                            Ok(found) => {
                                if found > 0 {
                                    info!("found proof {}", found)
                                }
                            }
                            Err(e) => warn!("error in getting proofs {:?}", e),
                        }
                    }
                }
            });
            closer.handles.push(headers_handle);
        }

        let syncer = Syncer {
            store: wallet.store.clone(),
            master_blinding: wallet.master_blinding.clone(),
            network: network.clone(),
        };

        let tipper = Tipper {
            store: wallet.store.clone(),
            network: network.clone(),
        };

        let (close_tipper, r) = channel();
        closer.senders.push(close_tipper);
        let tipper_url = url.clone();
        let tipper_handle = thread::spawn(move || {
            info!("starting tipper thread");
            loop {
                if let Ok(client) = tipper_url.build_client() {
                    match tipper.tip(&client) {
                        Ok(current_tip) => {
                            if tip_height != current_tip {
                                tip_height = current_tip;
                                info!("tip is {:?}", tip_height);
                            }
                        }
                        Err(e) => {
                            warn!("exception in tipper {:?}", e);
                        }
                    }
                }
                if wait_or_close(&r, sync_interval) {
                    info!("closing tipper thread {:?}", tip_height);
                    break;
                }
            }
        });
        closer.handles.push(tipper_handle);

        let (close_syncer, r) = channel();
        closer.senders.push(close_syncer);
        let syncer_url = url.clone();
        let syncer_handle = thread::spawn(move || {
            info!("starting syncer thread");
            loop {
                match syncer_url.build_client() {
                    Ok(client) => match syncer.sync(&client) {
                        Ok(new_txs) => {
                            if new_txs {
                                info!("there are new transactions");
                            }
                        }
                        Err(e) => warn!("Error during sync, {:?}", e),
                    },
                    Err(e) => warn!("Can't build client {:?}", e),
                }
                if wait_or_close(&r, sync_interval) {
                    info!("closing syncer thread");
                    break;
                }
            }
        });
        closer.handles.push(syncer_handle);

        Ok(Self {
            data_root: data_root.to_string(),
            network,
            url,
            wallet: Some(wallet),
            closer,
        })
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        self.closer.close()?;
        Ok(())
    }

    fn get_wallet(&self) -> Result<&WalletCtx, Error> {
        self.wallet
            .as_ref()
            .ok_or_else(|| Error::Generic("wallet not initialized".into()))
    }

    pub fn block_status(&self) -> Result<(u32, BlockHash), Error> {
        let tip = self.get_wallet()?.get_tip()?;
        info!("tip={:?}", tip);
        Ok(tip)
    }

    pub fn tx_status(&self) -> Result<u64, Error> {
        let mut opt = GetTransactionsOpt::default();
        opt.count = 100;
        let txs = self.get_wallet()?.list_tx(&opt)?;
        let mut hasher = DefaultHasher::new();
        for tx in txs.iter() {
            std::hash::Hash::hash(&tx.txid, &mut hasher);
        }
        let status = hasher.finish();
        info!("txs.len={} status={}", txs.len(), status);
        Ok(status)
    }

    pub fn balance(&self) -> Result<Balances, Error> {
        self.get_wallet()?.balance()
    }

    pub fn address(&self) -> Result<AddressPointer, Error> {
        self.get_wallet()?.get_address()
    }

    pub fn transactions(&self, opt: &GetTransactionsOpt) -> Result<Vec<TransactionDetails>, Error> {
        self.get_wallet()?.list_tx(opt)
    }

    // actually should list all coins, not only the unspent ones
    pub fn utxos(&self) -> Result<Vec<TXO>, Error> {
        self.get_wallet()?.utxos()
    }

    pub fn create_tx(&self, opt: &mut CreateTransactionOpt) -> Result<TransactionDetails, Error> {
        self.get_wallet()?.create_tx(opt)
    }

    pub fn sign_tx(&self, transaction: &mut BETransaction, mnemonic: &str) -> Result<(), Error> {
        self.get_wallet()?.sign_with_mnemonic(transaction, mnemonic)
    }

    pub fn broadcast_tx(&self, transaction: &BETransaction) -> Result<(), Error> {
        info!("broadcast_transaction {:#?}", transaction.txid());
        let client = self.url.build_client()?;
        client.transaction_broadcast_raw(&transaction.serialize())?;
        Ok(())
    }
}
