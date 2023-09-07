use crate::config::Config;
use crate::error::Error;
use crate::model::{GetTransactionsOpt, TransactionDetails, UnblindedTXO, TXO};
use crate::store::{new_store, Store};
use crate::sync::Syncer;
use crate::util::p2shwpkh_script;
use electrum_client::ElectrumApi;
use elements;
use elements::bitcoin::hashes::hex::FromHex;
use elements::bitcoin::hashes::{sha256, Hash};
use elements::bitcoin::secp256k1::{All, PublicKey, Secp256k1};
use elements::bitcoin::util::bip32::{ChildNumber, ExtendedPubKey};
use elements::slip77::MasterBlindingKey;
use elements::{Address, AssetId, BlockHash, BlockHeader, OutPoint, Txid};
use hex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

pub struct ElectrumWallet {
    secp: Secp256k1<All>,
    config: Config,
    store: Store,
    xpub: ExtendedPubKey,
    master_blinding: MasterBlindingKey,
}

impl ElectrumWallet {
    /// Create a new regtest wallet
    pub fn new_regtest(
        policy_asset: &str,
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
        data_root: &str,
        xpub: &str,
        master_blinding_key: &str,
    ) -> Result<Self, Error> {
        let config =
            Config::new_regtest(tls, validate_domain, electrum_url, policy_asset, data_root)?;
        Self::new(config, xpub, master_blinding_key)
    }

    /// Create a new testnet wallet
    pub fn new_testnet(
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
        data_root: &str,
        xpub: &str,
        master_blinding_key: &str,
    ) -> Result<Self, Error> {
        let config = Config::new_testnet(tls, validate_domain, electrum_url, data_root)?;
        Self::new(config, xpub, master_blinding_key)
    }

    /// Create a new mainnet wallet
    pub fn new_mainnet(
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
        data_root: &str,
        xpub: &str,
        master_blinding_key: &str,
    ) -> Result<Self, Error> {
        let config = Config::new_mainnet(tls, validate_domain, electrum_url, data_root)?;
        Self::new(config, xpub, master_blinding_key)
    }

    fn new(config: Config, xpub: &str, master_blinding_key: &str) -> Result<Self, Error> {
        let secp = Secp256k1::new();
        let xpub = ExtendedPubKey::from_str(&xpub)?;

        let wallet_desc = format!("{}{:?}", xpub, config);
        let wallet_id = hex::encode(sha256::Hash::hash(wallet_desc.as_bytes()));

        let inner = elements::secp256k1_zkp::SecretKey::from_slice(
            &Vec::<u8>::from_hex(master_blinding_key).unwrap(),
        )
        .unwrap();
        let master_blinding = MasterBlindingKey(inner);

        let mut path: PathBuf = config.data_root().into();
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        path.push(wallet_id);
        let store = new_store(&path, xpub)?;

        Ok(ElectrumWallet {
            store,
            config,
            secp,
            xpub,
            master_blinding,
        })
    }

    /// Get the network policy asset
    pub fn policy_asset(&self) -> AssetId {
        self.config.policy_asset()
    }

    /// Sync the wallet transactions
    pub fn sync_txs(&self) -> Result<(), Error> {
        let syncer = Syncer {
            store: self.store.clone(),
            master_blinding: self.master_blinding.clone(),
        };

        if let Ok(client) = self.config.electrum_url().build_client() {
            match syncer.sync(&client) {
                Ok(true) => log::info!("there are new transcations"),
                Ok(false) => (),
                Err(e) => log::warn!("Error during sync, {:?}", e),
            }
        }
        Ok(())
    }

    /// Sync the blockchain tip
    pub fn sync_tip(&self) -> Result<(), Error> {
        if let Ok(client) = self.config.electrum_url().build_client() {
            let header = client.block_headers_subscribe_raw()?;
            let height = header.height as u32;
            let tip_height = self.store.read()?.cache.tip.0;
            if height != tip_height {
                let block_header: BlockHeader = elements::encode::deserialize(&header.header)?;
                let hash: BlockHash = block_header.block_hash();
                self.store.write()?.cache.tip = (height, hash);
            }
        }
        Ok(())
    }

    /// Get the blockchain tip
    pub fn tip(&self) -> Result<(u32, BlockHash), Error> {
        Ok(self.store.read()?.cache.tip)
    }

    /// Get a new wallet address
    pub fn address(&self) -> Result<Address, Error> {
        let pointer = {
            let store = &mut self.store.write()?.cache;
            store.indexes.external += 1;
            store.indexes.external
        };
        let path = [0, pointer];
        let path: Vec<ChildNumber> = path
            .iter()
            .map(|x| ChildNumber::Normal { index: *x })
            .collect();
        let derived = self.xpub.derive_pub(&self.secp, &path)?;
        let script = p2shwpkh_script(&derived.to_pub());
        let blinding_key = self.master_blinding.derive_blinding_key(&script);
        let public_key = PublicKey::from_secret_key(&self.secp, &blinding_key);
        let blinder = Some(public_key);
        let addr = Address::p2shwpkh(&derived.to_pub(), blinder, self.config.address_params());
        Ok(addr)
    }

    /// Get the wallet UTXOs
    pub fn utxos(&self) -> Result<Vec<UnblindedTXO>, Error> {
        let store_read = self.store.read()?;
        let mut txos = vec![];
        let spent = store_read.spent()?;
        for (tx_id, height) in store_read.cache.heights.iter() {
            let tx = store_read
                .cache
                .all_txs
                .get(tx_id)
                .ok_or_else(|| Error::Generic(format!("txos no tx {}", tx_id)))?;
            let tx_txos: Vec<UnblindedTXO> = {
                tx.output
                    .clone()
                    .into_iter()
                    .enumerate()
                    .map(|(vout, output)| {
                        (
                            OutPoint {
                                txid: tx.txid(),
                                vout: vout as u32,
                            },
                            output,
                        )
                    })
                    .filter(|(outpoint, _)| !spent.contains(&outpoint))
                    .filter_map(|(outpoint, output)| {
                        if let Some(unblinded) = store_read.cache.unblinded.get(&outpoint) {
                            let txo = TXO::new(outpoint, output.script_pubkey, height.clone());
                            return Some(UnblindedTXO {
                                txo: txo,
                                unblinded: unblinded.clone(),
                            });
                        }
                        None
                    })
                    .collect()
            };
            txos.extend(tx_txos);
        }
        txos.sort_by(|a, b| b.unblinded.value.cmp(&a.unblinded.value));

        Ok(txos)
    }

    /// Get the wallet balance
    pub fn balance(&self) -> Result<HashMap<AssetId, u64>, Error> {
        let mut result = HashMap::new();
        result.entry(self.config.policy_asset()).or_insert(0);
        for u in self.utxos()?.iter() {
            *result.entry(u.unblinded.asset).or_default() += u.unblinded.value;
        }
        Ok(result)
    }

    /// Get the wallet transactions
    pub fn transactions(&self, opt: &GetTransactionsOpt) -> Result<Vec<TransactionDetails>, Error> {
        let store_read = self.store.read()?;

        let mut txs = vec![];
        let mut my_txids: Vec<(&Txid, &Option<u32>)> = store_read.cache.heights.iter().collect();
        my_txids.sort_by(|a, b| {
            let height_cmp =
                b.1.unwrap_or(std::u32::MAX)
                    .cmp(&a.1.unwrap_or(std::u32::MAX));
            match height_cmp {
                Ordering::Equal => b.0.cmp(a.0),
                h @ _ => h,
            }
        });

        for (tx_id, height) in my_txids.iter().skip(opt.first).take(opt.count) {
            let tx = store_read
                .cache
                .all_txs
                .get(*tx_id)
                .ok_or_else(|| Error::Generic(format!("list_tx no tx {}", tx_id)))?;

            let tx_details = TransactionDetails::new(tx.clone(), **height);
            txs.push(tx_details);
        }

        Ok(txs)
    }
}
