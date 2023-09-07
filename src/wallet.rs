use crate::error::Error;
use crate::model::GetTransactionsOpt;
use crate::model::{TransactionDetails, UnblindedTXO, TXO};
use crate::network::{Config, ElementsNetwork};
use crate::scripts::p2shwpkh_script;
use crate::store::{new_store, Store};
use crate::sync::Syncer;
use bip39;
use electrum_client::ElectrumApi;
use elements;
use elements::bitcoin::hashes::{sha256, Hash};
use elements::bitcoin::secp256k1::{self, All, Secp256k1};
use elements::bitcoin::util::bip32::{
    ChildNumber, DerivationPath, ExtendedPrivKey, ExtendedPubKey,
};
use elements::slip77::MasterBlindingKey;
use elements::{Address, AssetId, BlockHash, BlockHeader, Txid};
use hex;
use log::{info, trace, warn};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

fn mnemonic2seed(mnemonic: &str) -> Result<Vec<u8>, Error> {
    let mnemonic = bip39::Mnemonic::parse_in(bip39::Language::English, mnemonic)?;
    let seed = mnemonic.to_seed("");
    Ok(seed.to_vec())
}

fn mnemonic2xprv(mnemonic: &str, config: Config) -> Result<ExtendedPrivKey, Error> {
    let seed = mnemonic2seed(mnemonic)?;
    let xprv = ExtendedPrivKey::new_master(
        elements::bitcoin::network::constants::Network::Testnet,
        &seed,
    )?;

    // BIP44: m / purpose' / coin_type' / account' / change / address_index
    // coin_type = 1776 liquid bitcoin as defined in https://github.com/satoshilabs/slips/blob/master/slip-0044.md
    // slip44 suggest 1 for every testnet, so we are using it also for regtest
    let coin_type: u32 = match config.network() {
        ElementsNetwork::Liquid => 1776,
        ElementsNetwork::LiquidTestnet => 1,
        ElementsNetwork::ElementsRegtest => 1,
    };
    // since we use P2WPKH-nested-in-P2SH it is 49 https://github.com/bitcoin/bips/blob/master/bip-0049.mediawiki
    let path_string = format!("m/49'/{}'/0'", coin_type);
    info!("Using derivation path {}/0|1/*", path_string);
    let path = DerivationPath::from_str(&path_string)?;
    let secp = Secp256k1::new();
    Ok(xprv.derive_priv(&secp, &path)?)
}

pub struct ElectrumWallet {
    pub secp: Secp256k1<All>,
    pub config: Config,
    pub store: Store,
    pub xpub: ExtendedPubKey,
    pub master_blinding: MasterBlindingKey,
    pub change_max_deriv: u32,
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
        let xprv = mnemonic2xprv(mnemonic, config.clone())?;
        let secp = Secp256k1::new();
        let xpub = ExtendedPubKey::from_priv(&secp, &xprv);

        let wallet_desc = format!("{}{:?}", xpub, config);
        let wallet_id = hex::encode(sha256::Hash::hash(wallet_desc.as_bytes()));

        let seed = mnemonic2seed(mnemonic)?;
        let master_blinding = MasterBlindingKey::new(&seed);

        let mut path: PathBuf = data_root.into();
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        path.push(wallet_id);
        info!("Store root path: {:?}", path);
        let store = new_store(&path, xpub)?;

        Ok(ElectrumWallet {
            store,
            config,
            secp,
            xpub,
            master_blinding,
            change_max_deriv: 0,
        })
    }

    pub fn network(&self) -> ElementsNetwork {
        self.config.network()
    }

    pub fn policy_asset(&self) -> AssetId {
        self.config.policy_asset()
    }

    fn update_tip(&self) -> Result<(), Error> {
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

    pub fn sync(&self) -> Result<(), Error> {
        let syncer = Syncer {
            store: self.store.clone(),
            master_blinding: self.master_blinding.clone(),
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
        Ok(self.store.read()?.cache.tip)
    }

    pub fn balance(&self) -> Result<HashMap<AssetId, u64>, Error> {
        self.sync()?;
        let mut result = HashMap::new();
        result.entry(self.config.policy_asset()).or_insert(0);
        for u in self.utxos()?.iter() {
            *result.entry(u.unblinded.asset).or_default() += u.unblinded.value;
        }
        Ok(result)
    }

    pub fn address(&self) -> Result<Address, Error> {
        self.sync()?;
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
        let public_key = secp256k1::PublicKey::from_secret_key(&self.secp, &blinding_key);
        let blinder = Some(public_key);
        let addr = elements::Address::p2shwpkh(
            &derived.to_pub(),
            blinder,
            self.config.network().address_params(),
        );
        Ok(addr)
    }

    pub fn transactions(&self, opt: &GetTransactionsOpt) -> Result<Vec<TransactionDetails>, Error> {
        self.sync()?;
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
            trace!("tx_id {}", tx_id);

            let tx = store_read
                .cache
                .all_txs
                .get(*tx_id)
                .ok_or_else(|| Error::Generic(format!("list_tx no tx {}", tx_id)))?;

            let tx_details = TransactionDetails::new(tx.clone(), **height);

            txs.push(tx_details);
        }
        info!(
            "list_tx {:?}",
            txs.iter().map(|e| &e.txid).collect::<Vec<&String>>()
        );

        Ok(txs)
    }

    // actually should list all coins, not only the unspent ones
    pub fn utxos(&self) -> Result<Vec<UnblindedTXO>, Error> {
        self.sync()?;
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
                            elements::OutPoint {
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
}
