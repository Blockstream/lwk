mod error;
mod interface;
mod model;
mod network;
mod scripts;
mod store;
mod sync;
mod transaction;
mod utils;

pub use crate::error::Error;
use crate::interface::WalletCtx;
pub use crate::model::{
    CreateTransactionOpt, Destination, GetTransactionsOpt, TransactionDetails, UnblindedTXO, TXO,
};
use crate::network::Config;
pub use crate::network::ElementsNetwork;
use crate::sync::Syncer;
pub use crate::utils::tx_to_hex;
use electrum_client::ElectrumApi;
use elements::{Address, AssetId, BlockHash, BlockHeader, Transaction};
use log::{info, warn};
use std::collections::HashMap;

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
}
