mod error;
mod interface;
mod model;
mod network;
mod scripts;
mod store;
mod transaction;
mod utils;

pub use crate::error::Error;
use crate::interface::WalletCtx;
pub use crate::model::{
    CreateTransactionOpt, Destination, GetTransactionsOpt, TransactionDetails, UnblindedTXO, TXO,
};
use crate::network::Config;
pub use crate::network::ElementsNetwork;
use crate::store::{Indexes, Store, BATCH_SIZE};
use crate::transaction::*;
pub use crate::utils::tx_to_hex;
use electrum_client::GetHistoryRes;
use electrum_client::{Client, ElectrumApi};
use elements::bitcoin::hashes::hex::ToHex;
use elements::bitcoin::secp256k1::Secp256k1;
use elements::bitcoin::util::bip32::DerivationPath;
use elements::bitcoin::{Script as BitcoinScript, Txid as BitcoinTxid};
use elements::confidential::{Asset, Nonce, Value};
use elements::slip77::MasterBlindingKey;
use elements::{
    Address, AssetId, BlockHash, BlockHeader, OutPoint, Script, Transaction, TxOut, TxOutSecrets,
    Txid,
};
use log::{debug, info, trace, warn};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::{HashMap, HashSet};

struct Syncer {
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
        debug!("start sync");

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
                let scripts_bitcoin: Vec<&BitcoinScript> =
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
                    let txid = Txid::from_hash(el.tx_hash.as_hash());
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

        let store_indexes = self.store.read()?.cache.indexes.clone();

        let changed =
            if !new_txs.txs.is_empty() || store_indexes != last_used || !scripts.is_empty() {
                info!(
                    "There are changes in the store new_txs:{:?} txid_height:{:?}",
                    new_txs.txs.iter().map(|tx| tx.0).collect::<Vec<Txid>>(),
                    txid_height
                );
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
                .map(|t| BitcoinTxid::from_hash(t.as_hash()))
                .collect();
            let txs_bitcoin: Vec<&BitcoinTxid> = txs_bitcoin.iter().map(|t| t).collect();
            let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_bitcoin)?;
            let mut txs_downloaded: Vec<Transaction> = vec![];
            for vec in txs_bytes_downloaded {
                let tx: Transaction = elements::encode::deserialize(&vec)?;
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
                        let outpoint = OutPoint {
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
                let txs_bitcoin: Vec<BitcoinTxid> = txs_to_download
                    .iter()
                    .map(|t| BitcoinTxid::from_hash(t.as_hash()))
                    .collect();
                let txs_bitcoin: Vec<&BitcoinTxid> = txs_bitcoin.iter().map(|t| t).collect();
                let txs_bytes_downloaded = client.batch_transaction_get_raw(txs_bitcoin)?;
                for vec in txs_bytes_downloaded {
                    let mut tx: Transaction = elements::encode::deserialize(&vec)?;
                    strip_witness(&mut tx);
                    txs.push((tx.txid(), tx));
                }
            }
            Ok(DownloadTxResult { txs, unblinds })
        } else {
            Ok(DownloadTxResult::default())
        }
    }

    pub fn try_unblind(&self, outpoint: OutPoint, output: TxOut) -> Result<TxOutSecrets, Error> {
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

                info!(
                    "Unblinded outpoint:{} asset:{} value:{}",
                    outpoint,
                    &txout_secrets.asset.to_hex(),
                    txout_secrets.value,
                );

                Ok(txout_secrets)
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
        data_root: &str,
        mnemonic: &str,
    ) -> Result<Self, Error> {
        let config = Config::new_regtest(tls, validate_domain, electrum_url, policy_asset)?;
        Self::new(config, data_root, mnemonic)
    }

    pub fn new_testnet(
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
        data_root: &str,
        mnemonic: &str,
    ) -> Result<Self, Error> {
        let config = Config::new_testnet(tls, validate_domain, electrum_url)?;
        Self::new(config, data_root, mnemonic)
    }

    pub fn new_mainnet(
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
        data_root: &str,
        mnemonic: &str,
    ) -> Result<Self, Error> {
        let config = Config::new_mainnet(tls, validate_domain, electrum_url)?;
        Self::new(config, data_root, mnemonic)
    }

    fn new(config: Config, data_root: &str, mnemonic: &str) -> Result<Self, Error> {
        let wallet = WalletCtx::from_mnemonic(mnemonic, &data_root, config.clone())?;

        Ok(Self { config, wallet })
    }

    pub fn network(&self) -> ElementsNetwork {
        self.config.network()
    }

    pub fn policy_asset(&self) -> AssetId {
        self.wallet.config.policy_asset()
    }

    fn update_tip(&self) -> Result<(), Error> {
        if let Ok(client) = self.config.electrum_url().build_client() {
            let header = client.block_headers_subscribe_raw()?;
            let height = header.height as u32;
            let tip_height = self.wallet.store.read()?.cache.tip.0;
            if height != tip_height {
                let block_header: BlockHeader = elements::encode::deserialize(&header.header)?;
                let hash: BlockHash = block_header.block_hash();
                self.wallet.store.write()?.cache.tip = (height, hash);
            }
        }
        Ok(())
    }

    pub fn sync(&self) -> Result<(), Error> {
        let syncer = Syncer {
            store: self.wallet.store.clone(),
            master_blinding: self.wallet.master_blinding.clone(),
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

    pub fn balance(&self) -> Result<HashMap<AssetId, u64>, Error> {
        self.sync()?;
        self.wallet.balance()
    }

    pub fn address(&self) -> Result<Address, Error> {
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

    pub fn sign_tx(&self, transaction: &mut Transaction, mnemonic: &str) -> Result<(), Error> {
        self.wallet.sign_with_mnemonic(transaction, mnemonic)
    }

    pub fn broadcast_tx(&self, transaction: &Transaction) -> Result<(), Error> {
        info!("broadcast_transaction {:#?}", transaction.txid());
        let client = self.config.electrum_url().build_client()?;
        client.transaction_broadcast_raw(&elements::encode::serialize(transaction))?;
        Ok(())
    }
}
