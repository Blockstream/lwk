mod error;
mod headers;
mod interface;
mod model;
mod network;
mod scripts;
mod store;
mod transaction;

pub use crate::error::Error;
pub use crate::model::{
    CreateTransactionOpt, Destination, GetTransactionsOpt, SPVVerifyResult, TransactionDetails,
    Unblinded, UnblindedTXO, TXO,
};

use network::*;

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::time::Instant;

use crate::headers::Verifier;
use crate::interface::{make_shared_secret, parse_rangeproof_message, WalletCtx};
use crate::model::*;
use crate::network::Config;
use crate::store::{Indexes, Store, BATCH_SIZE};
use crate::transaction::*;
use crate::ElementsNetwork;

use log::{debug, info, trace, warn};

use elements::bitcoin::hashes::hex::ToHex;
use elements::bitcoin::secp256k1;
use elements::bitcoin::util::bip32::DerivationPath;
use elements::{BlockHash, Script, Txid};

use elements;
use elements::confidential::{self, Asset, Nonce};
use elements::slip77::MasterBlindingKey;

use electrum_client::GetHistoryRes;
use electrum_client::{Client, ElectrumApi};

use rand::seq::SliceRandom;
use rand::thread_rng;

struct Syncer {
    pub store: Store,
    pub master_blinding: MasterBlindingKey,
    pub config: Config,
    secp: secp256k1::Secp256k1<secp256k1::All>,
}

struct Tipper {
    pub store: Store,
    pub config: Config,
}

struct Headers {
    pub store: Store,
    pub verifier: Verifier,
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
            let block_header: elements::BlockHeader =
                elements::encode::deserialize(&header.header)?;
            let hash: BlockHash = block_header.block_hash();
            info!("saving in store new tip {:?}", (height, hash));
            self.store.write()?.cache.tip = (height, hash);
        }
        Ok(height)
    }
}

impl Headers {
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
            let proof = client.transaction_get_merkle(
                &elements::bitcoin::Txid::from_hash(txid.as_hash()),
                height as usize,
            )?;
            let verified = if let Some(header) = self.store.read()?.cache.headers.get(&height) {
                self.verifier.verify_tx_proof(&txid, proof, &header).is_ok()
            } else {
                false
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
}

#[derive(Default)]
struct DownloadTxResult {
    txs: Vec<(Txid, elements::Transaction)>,
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
                let scripts_bitcoin: Vec<elements::bitcoin::Script> = batch
                    .value
                    .iter()
                    .map(|e| elements::bitcoin::Script::from(e.0.clone().into_bytes()))
                    .collect();
                let scripts_bitcoin: Vec<&elements::bitcoin::Script> =
                    scripts_bitcoin.iter().map(|e| e).collect();
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
                    let txid = elements::Txid::from_hash(el.tx_hash.as_hash());
                    if height == 0 {
                        txid_height.insert(txid, None);
                    } else {
                        txid_height.insert(txid, Some(height as u32));
                    }

                    history_txs_id.insert(txid);
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
    ) -> Result<Vec<(u32, elements::BlockHeader)>, Error> {
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
            let mut headers_downloaded: Vec<elements::BlockHeader> = vec![];
            for vec in headers_bytes_downloaded {
                headers_downloaded.push(elements::encode::deserialize(&vec)?);
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
            let txs_bitcoin: Vec<elements::bitcoin::Txid> = txs_to_download
                .iter()
                .map(|t| elements::bitcoin::Txid::from_hash(t.as_hash()))
                .collect();
            let txs_bitcoin: Vec<&elements::bitcoin::Txid> =
                txs_bitcoin.iter().map(|t| t).collect();
            let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_bitcoin)?;
            let mut txs_downloaded: Vec<elements::Transaction> = vec![];
            for vec in txs_bytes_downloaded {
                let tx: elements::Transaction = elements::encode::deserialize(&vec)?;
                txs_downloaded.push(tx);
            }
            info!("txs_downloaded {:?}", txs_downloaded.len());
            let previous_txs_to_download = HashSet::new();
            for mut tx in txs_downloaded.into_iter() {
                let txid = tx.txid();
                txs_in_db.insert(txid);

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
                strip_witness(&mut tx);
                txs.push((txid, tx));
            }

            let txs_to_download: Vec<&Txid> =
                previous_txs_to_download.difference(&txs_in_db).collect();
            if !txs_to_download.is_empty() {
                let txs_bitcoin: Vec<elements::bitcoin::Txid> = txs_to_download
                    .iter()
                    .map(|t| elements::bitcoin::Txid::from_hash(t.as_hash()))
                    .collect();
                let txs_bitcoin: Vec<&elements::bitcoin::Txid> =
                    txs_bitcoin.iter().map(|t| t).collect();
                let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_bitcoin)?;
                for vec in txs_bytes_downloaded {
                    let mut tx: elements::Transaction = elements::encode::deserialize(&vec)?;
                    strip_witness(&mut tx);
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
                let asset_commitment =
                    secp256k1_zkp::Generator::from_slice(&output.asset.commitment().unwrap())?;
                let value_commitment = secp256k1_zkp::PedersenCommitment::from_slice(
                    &output.value.commitment().unwrap(),
                )?;
                let sender_pk =
                    secp256k1::PublicKey::from_slice(&output.nonce.commitment().unwrap())?;
                let rangeproof = secp256k1_zkp::RangeProof::from_slice(&output.witness.rangeproof)?;

                let receiver_sk = self
                    .master_blinding
                    .derive_blinding_key(&output.script_pubkey);
                let shared_secret = make_shared_secret(&sender_pk, &receiver_sk);

                let (opening, _) = rangeproof.rewind(
                    &self.secp,
                    value_commitment,
                    shared_secret,
                    output.script_pubkey.as_bytes(),
                    asset_commitment,
                )?;

                let (asset, asset_blinder) = parse_rangeproof_message(&*opening.message)?;

                info!(
                    "Unblinded outpoint:{} asset:{} value:{}",
                    outpoint,
                    &asset.to_hex(),
                    opening.value,
                );

                let unblinded = Unblinded {
                    asset,
                    value: opening.value,
                    asset_blinder,
                    value_blinder: opening.blinding_factor,
                };
                Ok(unblinded)
            }
            _ => Err(Error::Generic(
                "received unconfidential or null asset/value/nonce".into(),
            )),
        }
    }
}

pub struct ElectrumWallet {
    config: Config,
    wallet: WalletCtx,
}

impl ElectrumWallet {
    pub fn new_regtest(
        policy_asset: &str,
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
        spv_enabled: bool,
        data_root: &str,
        mnemonic: &str,
    ) -> Result<Self, Error> {
        let config = Config::new_regtest(
            tls,
            validate_domain,
            spv_enabled,
            electrum_url,
            policy_asset,
        )?;
        Self::new(config, data_root, mnemonic)
    }

    pub fn new_mainnet(
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
        spv_enabled: bool,
        data_root: &str,
        mnemonic: &str,
    ) -> Result<Self, Error> {
        let config = Config::new_mainnet(tls, validate_domain, spv_enabled, electrum_url)?;
        Self::new(config, data_root, mnemonic)
    }

    fn new(config: Config, data_root: &str, mnemonic: &str) -> Result<Self, Error> {
        let wallet = WalletCtx::from_mnemonic(mnemonic, &data_root, config.clone())?;

        Ok(Self { config, wallet })
    }

    pub fn update_fee_estimates(&self) {
        info!("building client");
        if let Ok(fee_client) = self.config.electrum_url().build_client() {
            info!("building built end");
            let fee_store = self.wallet.store.clone();
            match try_get_fee_estimates(&fee_client) {
                Ok(fee_estimates) => fee_store.write().unwrap().cache.fee_estimates = fee_estimates,
                Err(e) => warn!("can't update fee estimates {:?}", e),
            };
        }
    }

    fn update_tip(&self) -> Result<(), Error> {
        // consider not using Tipper
        let tipper = Tipper {
            store: self.wallet.store.clone(),
            config: self.config.clone(),
        };
        let tipper_url = self.config.electrum_url();
        if let Ok(client) = tipper_url.build_client() {
            match tipper.tip(&client) {
                Ok(_) => (),
                Err(e) => {
                    warn!("exception in tipper {:?}", e);
                }
            }
        }
        Ok(())
    }

    pub fn update_spv(&self) -> Result<(), Error> {
        let verifier = Verifier::new(self.config.network());

        let mut headers = Headers {
            store: self.wallet.store.clone(),
            verifier,
        };

        self.update_tip()?;
        if let Ok(client) = self.config.electrum_url().build_client() {
            info!("getting proofs");
            match headers.get_proofs(&client) {
                Ok(found) => {
                    if found > 0 {
                        info!("found proof {}", found)
                    }
                }
                Err(e) => warn!("error in getting proofs {:?}", e),
            }
        }
        Ok(())
    }

    pub fn sync(&self) -> Result<(), Error> {
        let syncer = Syncer {
            store: self.wallet.store.clone(),
            master_blinding: self.wallet.master_blinding.clone(),
            config: self.config.clone(),
            secp: secp256k1::Secp256k1::new(),
        };

        if let Ok(client) = self.config.electrum_url().build_client() {
            match syncer.sync(&client) {
                Ok(true) => info!("there are new transcations"),
                Ok(false) => (),
                Err(e) => warn!("Error during sync, {:?}", e),
            }
        }
        Ok(())
    }

    pub fn block_status(&self) -> Result<(u32, BlockHash), Error> {
        self.update_tip()?;
        let tip = self.wallet.get_tip()?;
        info!("tip={:?}", tip);
        Ok(tip)
    }

    pub fn tx_status(&self) -> Result<u64, Error> {
        self.sync()?;
        let mut opt = GetTransactionsOpt::default();
        opt.count = 100;
        let txs = self.wallet.list_tx(&opt)?;
        let mut hasher = DefaultHasher::new();
        for tx in txs.iter() {
            std::hash::Hash::hash(&tx.txid, &mut hasher);
        }
        let status = hasher.finish();
        info!("txs.len={} status={}", txs.len(), status);
        Ok(status)
    }

    pub fn balance(&self) -> Result<HashMap<elements::issuance::AssetId, u64>, Error> {
        self.sync()?;
        self.wallet.balance()
    }

    pub fn address(&self) -> Result<elements::Address, Error> {
        self.sync()?;
        self.wallet.get_address()
    }

    pub fn transactions(&self, opt: &GetTransactionsOpt) -> Result<Vec<TransactionDetails>, Error> {
        self.sync()?;
        self.wallet.list_tx(opt)
    }

    // actually should list all coins, not only the unspent ones
    pub fn utxos(&self) -> Result<Vec<UnblindedTXO>, Error> {
        self.sync()?;
        self.wallet.utxos()
    }

    pub fn create_tx(&self, opt: &mut CreateTransactionOpt) -> Result<TransactionDetails, Error> {
        self.sync()?;
        self.wallet.create_tx(opt)
    }

    pub fn sign_tx(
        &self,
        transaction: &mut elements::Transaction,
        mnemonic: &str,
    ) -> Result<(), Error> {
        self.wallet.sign_with_mnemonic(transaction, mnemonic)
    }

    pub fn broadcast_tx(&self, transaction: &elements::Transaction) -> Result<(), Error> {
        info!("broadcast_transaction {:#?}", transaction.txid());
        let client = self.config.electrum_url().build_client()?;
        client.transaction_broadcast_raw(&elements::encode::serialize(transaction))?;
        Ok(())
    }
}
