use crate::bitcoin::bip32::Fingerprint;
use crate::clients::{try_unblind, LastUnused};
use crate::config::{Config, ElementsNetwork};
use crate::descriptor::Chain;
use crate::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use crate::elements::pset::PartiallySignedTransaction;
use crate::elements::secp256k1_zkp::ZERO_TWEAK;
use crate::elements::{AssetId, BlockHash, OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::error::Error;
use crate::hashes::Hash;
use crate::model::{
    AddressResult, BitcoinAddressResult, ExternalUtxo, IssuanceDetails, WalletTx, WalletTxOut,
};
use crate::persister::PersistError;
use crate::store::{Height, ScriptBatch, Store, Timestamp, BATCH_SIZE};
use crate::tx_builder::{extract_issuances, WolletTxBuilder};
use crate::util::EC;
use crate::{BlindingPublicKey, FsPersister, NoPersist, Persister, Update, WolletDescriptor};
use elements::bitcoin::bip32::ChildNumber;
use elements::{bitcoin, Address, AddressParams};
use elements_miniscript::psbt::PsbtExt;
use elements_miniscript::{BtcDescriptor, ForEachKey};
use elements_miniscript::{
    ConfidentialDescriptor, DefiniteDescriptorKey, Descriptor, DescriptorPublicKey,
};
use fxhash::FxHasher;
use lwk_common::{
    burn_script, pset_balance, pset_issuances, pset_signatures, Balance, PsetDetails,
};
use std::cmp::Ordering;
use std::collections::{hash_map::Entry, BTreeMap, HashMap, HashSet};
use std::hash::Hasher;
use std::path::Path;
use std::sync::{atomic, Arc};

/// A watch-only wallet defined by a CT descriptor.
pub struct Wollet {
    pub(crate) config: Config,
    pub(crate) store: Store,
    pub(crate) persister: Arc<dyn Persister + Send + Sync>,
    pub(crate) descriptor: WolletDescriptor,
    // cached value
    max_weight_to_satisfy: usize,
}

/// A builder for constructing [`Wollet`] instances
pub struct WolletBuilder {
    network: ElementsNetwork,
    descriptor: WolletDescriptor,
    persister: Arc<dyn Persister + Send + Sync>,
}

impl WolletBuilder {
    /// Create a `Wollet` builder
    pub fn new(network: ElementsNetwork, descriptor: WolletDescriptor) -> Self {
        Self {
            network,
            descriptor,
            persister: Arc::new(NoPersist {}),
        }
    }

    /// Specify the `Wollet` persister
    pub fn with_persister(mut self, persister: Arc<dyn Persister + Send + Sync>) -> Self {
        self.persister = persister;
        self
    }

    /// Build the `Wollet`
    pub fn build(self) -> Result<Wollet, Error> {
        let config = Config::new(self.network)?;

        let store = Store::default();
        let max_weight_to_satisfy = self
            .descriptor
            .definite_descriptor(Chain::External, 0)?
            .max_weight_to_satisfy()?;
        let mut wollet = Wollet {
            store,
            config,
            descriptor: self.descriptor,
            persister: self.persister,
            max_weight_to_satisfy,
        };

        for i in 0.. {
            match wollet.persister.get(i)? {
                Some(update) => wollet.apply_update_no_persist(update)?,
                None => break,
            }
        }

        Ok(wollet)
    }
}

/// A coincise state of the wallet, in particular having only transactions ids instead of full
/// transactions and missing other things not strictly needed for a scan.
/// By using this instead of a borrow of the wallet we can release locks
pub struct WolletConciseState {
    wollet_status: u64,
    descriptor: WolletDescriptor,
    txs: HashSet<Txid>,
    paths: HashMap<Script, (Chain, ChildNumber)>,
    scripts: HashMap<(Chain, ChildNumber), (Script, BlindingPublicKey)>,
    heights: HashMap<Txid, Option<Height>>,
    tip: (Height, BlockHash),
    last_unused: LastUnused,
}

pub trait WolletState {
    fn get_script_batch(
        &self,
        batch: u32,
        descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    ) -> Result<ScriptBatch, Error>;
    fn get_or_derive(
        &self,
        ext_int: Chain,
        child: ChildNumber,
        descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    ) -> Result<(Script, BlindingPublicKey, bool), Error>;
    fn heights(&self) -> &HashMap<Txid, Option<Height>>;
    fn paths(&self) -> &HashMap<Script, (Chain, ChildNumber)>;
    fn txs(&self) -> HashSet<Txid>;
    fn tip(&self) -> (Height, BlockHash);
    fn last_unused(&self) -> LastUnused; // TODO change to &LastUnused when possible
    fn descriptor(&self) -> WolletDescriptor;
    fn wollet_status(&self) -> u64;
}

impl WolletState for WolletConciseState {
    // TODO duplicated from Wollet
    fn get_script_batch(
        &self,
        batch: u32,
        descriptor: &ConfidentialDescriptor<DescriptorPublicKey>, // non confidential (we need only script_pubkey), non multipath (we need to be able to derive with index)
    ) -> Result<ScriptBatch, Error> {
        let mut result = ScriptBatch {
            cached: true,
            ..Default::default()
        };

        let start = batch * BATCH_SIZE;
        let end = start + BATCH_SIZE;
        let ext_int: Chain = descriptor.try_into().unwrap_or(Chain::External);
        for j in start..end {
            let child = ChildNumber::from_normal_idx(j)?;
            let (script, blinding_pubkey, cached) =
                self.get_or_derive(ext_int, child, descriptor)?;
            result.cached = cached;
            result
                .value
                .push((script, (ext_int, child, blinding_pubkey)));
        }

        Ok(result)
    }

    fn get_or_derive(
        &self,
        ext_int: Chain,
        child: ChildNumber,
        descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    ) -> Result<(Script, BlindingPublicKey, bool), Error> {
        let opt_script = self.scripts.get(&(ext_int, child));
        let (script, blinding_pubkey, cached) = match opt_script {
            Some((script, blinding_pubkey)) => (script.clone(), *blinding_pubkey, true),
            None => {
                let (script, blinding_pubkey) = derive_script_and_blinding_key(descriptor, child)?;
                (script, blinding_pubkey, false)
            }
        };
        Ok((script, blinding_pubkey, cached))
    }

    fn heights(&self) -> &HashMap<Txid, Option<Height>> {
        &self.heights
    }

    fn paths(&self) -> &HashMap<Script, (Chain, ChildNumber)> {
        &self.paths
    }

    fn txs(&self) -> HashSet<Txid> {
        self.txs.clone()
    }

    fn tip(&self) -> (Height, BlockHash) {
        self.tip
    }

    fn last_unused(&self) -> LastUnused {
        self.last_unused.clone()
    }

    fn descriptor(&self) -> WolletDescriptor {
        self.descriptor.clone()
    }

    fn wollet_status(&self) -> u64 {
        self.wollet_status
    }
}

impl std::fmt::Debug for Wollet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "wollet({:?})", self.descriptor)
    }
}

impl WolletState for Wollet {
    fn get_script_batch(
        &self,
        batch: u32,
        descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    ) -> Result<ScriptBatch, Error> {
        self.store.get_script_batch(batch, descriptor)
    }

    fn get_or_derive(
        &self,
        ext_int: Chain,
        child: ChildNumber,
        descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    ) -> Result<(Script, BlindingPublicKey, bool), Error> {
        self.store.get_or_derive(ext_int, child, descriptor)
    }

    fn heights(&self) -> &HashMap<Txid, Option<Height>> {
        &self.store.cache.heights
    }

    fn paths(&self) -> &HashMap<Script, (Chain, ChildNumber)> {
        &self.store.cache.paths
    }

    fn txs(&self) -> HashSet<Txid> {
        self.store.cache.all_txs.keys().cloned().collect()
    }

    fn tip(&self) -> (Height, BlockHash) {
        self.store.cache.tip
    }

    fn last_unused(&self) -> LastUnused {
        // TODO use LastUnused internally in Wollet
        LastUnused {
            internal: self.last_unused_internal(),
            external: self.last_unused_external(),
        }
    }

    fn descriptor(&self) -> WolletDescriptor {
        self.wollet_descriptor()
    }

    fn wollet_status(&self) -> u64 {
        self.status()
    }
}

impl std::hash::Hash for Wollet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.config.hash(state);
        self.store.hash(state);
        self.descriptor.hash(state);
    }
}

impl Wollet {
    /// Create a new  wallet
    pub fn new(
        network: ElementsNetwork,
        persister: Arc<dyn Persister + Send + Sync>,
        descriptor: WolletDescriptor,
    ) -> Result<Self, Error> {
        WolletBuilder::new(network, descriptor)
            .with_persister(persister)
            .build()
    }

    /// Whether the wallet is segwit (BIP141)
    pub fn is_segwit(&self) -> bool {
        self.descriptor()
            .descriptor
            .desc_type()
            .segwit_version()
            .is_some()
    }

    /// Whether the wallet is AMP0
    #[cfg(feature = "amp0")]
    pub fn is_amp0(&self) -> bool {
        self.descriptor.is_amp0()
    }

    /// Max weight to satisfy for inputs belonging to this wallet
    pub fn max_weight_to_satisfy(&self) -> usize {
        self.max_weight_to_satisfy
    }

    /// Get a concise state of the wallet, allowing to perform a scan (like [`crate::clients::blocking::BlockchainBackend::full_scan()`]) without holding the lock on the wallet.
    pub fn state(&self) -> WolletConciseState {
        let cache = &self.store.cache;
        WolletConciseState {
            wollet_status: self.status(),
            descriptor: self.wollet_descriptor(),
            txs: cache.all_txs.keys().cloned().collect(),
            paths: cache.paths.clone(),
            scripts: cache.scripts.clone(),
            heights: cache.heights.clone(),
            tip: cache.tip,
            last_unused: LastUnused {
                internal: cache.last_unused_internal.load(atomic::Ordering::Relaxed),
                external: cache.last_unused_external.load(atomic::Ordering::Relaxed),
            },
        }
    }

    /// Create a new wallet persisting on file system
    pub fn with_fs_persist<P: AsRef<Path>>(
        network: ElementsNetwork,
        descriptor: WolletDescriptor,
        datadir: P,
    ) -> Result<Self, Error> {
        Self::new(
            network,
            FsPersister::new(datadir, network, &descriptor)?,
            descriptor,
        )
    }

    /// Create a new wallet which not persist anything
    pub fn without_persist(
        network: ElementsNetwork,
        descriptor: WolletDescriptor,
    ) -> Result<Self, Error> {
        Self::new(network, Arc::new(NoPersist {}), descriptor)
    }

    /// Get the network policy asset
    pub fn policy_asset(&self) -> AssetId {
        self.config.policy_asset()
    }

    /// Creates a transaction builder with a reference to this wallet
    pub fn tx_builder(&self) -> WolletTxBuilder {
        WolletTxBuilder::new(self)
    }

    /// Get the network
    pub fn network(&self) -> ElementsNetwork {
        self.config.network()
    }

    /// Get a reference of the wallet descriptor
    pub fn descriptor(&self) -> &ConfidentialDescriptor<DescriptorPublicKey> {
        self.descriptor.as_ref()
    }

    /// Get a copy of the wallet descriptor
    pub fn wollet_descriptor(&self) -> WolletDescriptor {
        self.descriptor.clone()
    }

    /// Get the blockchain tip
    pub fn tip(&self) -> Tip {
        let (height, hash) = self.store.cache.tip;
        let timestamp = self.store.cache.timestamps.get(&height).cloned();
        Tip {
            height,
            hash,
            timestamp,
        }
    }

    /// Get a wallet address
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn address(&self, index: Option<u32>) -> Result<AddressResult, Error> {
        if index.is_some() && !self.descriptor.has_wildcard() {
            // TODO: this error should be upstreamed to at_derivation_index https://github.com/rust-bitcoin/rust-miniscript/issues/829
            return Err(Error::IndexWithoutWildcard);
        }

        let index = self.unwrap_or_last_unused(index);

        let address = self
            .descriptor
            .address(index, self.config.address_params())?;
        Ok(AddressResult::new(address, index))
    }

    /// Get a wallet pegin address
    ///
    /// A pegin address is a bitcoin address, funds sent to this address are
    /// converted to liquid bitcoins.
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn pegin_address(
        &self,
        index: Option<u32>,
        fed_desc: BtcDescriptor<bitcoin::PublicKey>,
    ) -> Result<BitcoinAddressResult, Error> {
        let index = self.unwrap_or_last_unused(index);
        let network = match self.network() {
            ElementsNetwork::Liquid => bitcoin::Network::Bitcoin,
            ElementsNetwork::LiquidTestnet => bitcoin::Network::Testnet,
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => bitcoin::Network::Regtest,
        };

        let address = self.descriptor.pegin_address(index, network, fed_desc)?;
        Ok(BitcoinAddressResult::new(address, index))
    }

    pub(crate) fn last_unused_external(&self) -> u32 {
        let cache = &self.store.cache;
        cache.last_unused_external.load(atomic::Ordering::Relaxed)
    }

    pub(crate) fn last_unused_internal(&self) -> u32 {
        let cache = &self.store.cache;
        cache.last_unused_internal.load(atomic::Ordering::Relaxed)
    }

    /// Returns the given `index` unwrapped if Some, otherwise
    /// takes the last unused external index of the wallet
    fn unwrap_or_last_unused(&self, index: Option<u32>) -> u32 {
        match index {
            Some(i) => i,
            None => self.last_unused_external(),
        }
    }

    /// Get a wallet change address
    ///
    /// If a specific descriptor is given for change addresses  it's used to derive this address
    /// Otherwise this is the same as `address()`
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn change(&self, index: Option<u32>) -> Result<AddressResult, Error> {
        let index = match index {
            Some(i) => i,
            None => self.last_unused_internal(),
        };

        let address = self
            .descriptor
            .change(index, self.config.address_params())?;
        Ok(AddressResult::new(address, index))
    }

    fn utxos_inner(&self) -> Result<Vec<WalletTxOut>, Error> {
        Ok(self
            .txos_inner()?
            .into_iter()
            .filter(|txo| !txo.is_spent)
            .collect())
    }

    fn txos_inner(&self) -> Result<Vec<WalletTxOut>, Error> {
        let mut txos = vec![];
        let spent = self.store.spent()?;
        for (tx_id, height) in self.store.cache.heights.iter() {
            let tx = self
                .store
                .cache
                .all_txs
                .get(tx_id)
                .ok_or_else(|| Error::Generic(format!("txos no tx {tx_id}")))?;
            let tx_txos = tx
                .output
                .iter()
                .enumerate()
                .map(|(vout, output)| {
                    let out_point = OutPoint {
                        txid: *tx_id,
                        vout: vout as u32,
                    };
                    (out_point, output, spent.contains(&out_point))
                })
                .filter_map(|(outpoint, output, is_spent)| {
                    if let Some(unblinded) = self.store.cache.unblinded.get(&outpoint) {
                        let index = self.index(&output.script_pubkey).ok()?;
                        let blinding_pubkey = self
                            .store
                            .cache
                            .scripts
                            .get(&(index.0, index.1.into()))
                            .map(|(_, blinding_pubkey)| *blinding_pubkey);
                        let address = Address::from_script(
                            &output.script_pubkey,
                            blinding_pubkey,
                            self.network().address_params(),
                        )?;
                        return Some(WalletTxOut {
                            outpoint,
                            script_pubkey: output.script_pubkey.clone(),
                            height: *height,
                            unblinded: *unblinded,
                            wildcard_index: index.1,
                            ext_int: index.0,
                            is_spent,
                            address,
                        });
                    }
                    None
                });
            txos.extend(tx_txos);
        }

        Ok(txos)
    }

    /// Get the wallet UTXOs
    pub fn utxos(&self) -> Result<Vec<WalletTxOut>, Error> {
        let mut utxos = self.utxos_inner()?;
        utxos.sort_by(|a, b| b.unblinded.value.cmp(&a.unblinded.value));
        Ok(utxos)
    }

    /// Get the wallet outputs, including spent ones
    pub fn txos(&self) -> Result<Vec<WalletTxOut>, Error> {
        self.txos_inner()
    }

    pub(crate) fn txos_map(&self) -> Result<HashMap<OutPoint, WalletTxOut>, Error> {
        Ok(self
            .txos_inner()?
            .into_iter()
            .map(|txo| (txo.outpoint, txo))
            .collect())
    }

    pub(crate) fn utxos_map(&self) -> Result<HashMap<OutPoint, WalletTxOut>, Error> {
        Ok(self
            .utxos_inner()?
            .into_iter()
            .map(|txo| (txo.outpoint, txo))
            .collect())
    }
    /// Get the explicit UTXOs sent to script pubkeys owned by the wallet
    ///
    /// They can be spent as external utxos using [`crate::TxBuilder::add_external_utxos()`].
    pub fn explicit_utxos(&self) -> Result<Vec<ExternalUtxo>, Error> {
        let spent = self.store.spent()?;
        let mut utxos = vec![];
        for (txid, tx) in self.store.cache.all_txs.iter() {
            for (vout, o) in tx.output.iter().enumerate() {
                let outpoint = OutPoint::new(*txid, vout as u32);
                if !o.script_pubkey.is_empty()
                    && o.asset.is_explicit()
                    && o.value.is_explicit()
                    && self.store.cache.paths.contains_key(&o.script_pubkey)
                    && !spent.contains(&outpoint)
                {
                    let unblinded = TxOutSecrets::new(
                        o.asset.explicit().expect("explicit"),
                        AssetBlindingFactor::zero(),
                        o.value.explicit().expect("explicit"),
                        ValueBlindingFactor::zero(),
                    );
                    let tx_ = if self.is_segwit() {
                        None
                    } else {
                        Some(tx.clone())
                    };
                    utxos.push(ExternalUtxo {
                        outpoint,
                        txout: o.clone(),
                        tx: tx_,
                        unblinded,
                        max_weight_to_satisfy: self.max_weight_to_satisfy,
                    });
                }
            }
        }
        Ok(utxos)
    }

    /// Extract the wallet UTXOs that a PSET is creating
    ///
    /// This function returns [`crate::model::ExternalUtxo`]s so it possible to spend them (using
    /// [`crate::TxBuilder::add_external_utxos()`]) without broadcasting the transaction.
    pub fn extract_wallet_utxos(
        &self,
        pset: &PartiallySignedTransaction,
    ) -> Result<Vec<ExternalUtxo>, Error> {
        let mut utxos = vec![];
        let tx = pset.extract_tx()?;
        let txid = tx.txid();
        for (vout, output) in pset.outputs().iter().enumerate() {
            if self.store.cache.paths.contains_key(&output.script_pubkey) {
                let outpoint = OutPoint::new(txid, vout as u32);
                // FIXME: also extract explicit utxos
                let txout = output.to_txout();
                if let Ok(unblinded) = try_unblind(&txout, &self.descriptor) {
                    let tx_ = if self.is_segwit() {
                        None
                    } else {
                        Some(tx.clone())
                    };
                    utxos.push(ExternalUtxo {
                        outpoint,
                        txout,
                        tx: tx_,
                        unblinded,
                        max_weight_to_satisfy: self.max_weight_to_satisfy,
                    });
                }
            }
        }
        Ok(utxos)
    }

    /// Get the transaction outputs that the wallet was unable to unbind
    ///
    /// In some particular situation they can be unblinded with [`crate::Wollet::reunblind()`].
    pub fn txos_cannot_unblind(&self) -> Result<Vec<OutPoint>, Error> {
        let mut txos = vec![];
        for (txid, tx) in self.store.cache.all_txs.iter() {
            for (vout, o) in tx.output.iter().enumerate() {
                let outpoint = OutPoint::new(*txid, vout as u32);
                if !o.script_pubkey.is_empty()
                    && self.store.cache.paths.contains_key(&o.script_pubkey)
                    && !self.store.cache.unblinded.contains_key(&outpoint)
                {
                    txos.push(outpoint);
                }
            }
        }
        Ok(txos)
    }

    /// Return UTXOs unblinded with a custom blinding key
    ///
    /// They can be spent using [`crate::TxBuilder::add_external_utxos()`]
    ///
    /// Note: if the blinding key is the one derived from the wallet descriptor,
    /// this function will NOT return that UTXO. That UTXO is available with the normal flow.
    pub fn unblind_utxos_with(
        &self,
        blinding_key: bitcoin::secp256k1::SecretKey,
    ) -> Result<Vec<ExternalUtxo>, Error> {
        let mut utxos = vec![];
        let spent = self.store.spent()?;
        let store_unblinded = &self.store.cache.unblinded;
        for (txid, tx) in self.store.cache.all_txs.iter() {
            for (i, txout) in tx.output.iter().enumerate() {
                if self.store.cache.paths.contains_key(&txout.script_pubkey) {
                    let outpoint = OutPoint::new(*txid, i as u32);
                    if !spent.contains(&outpoint) && !store_unblinded.contains_key(&outpoint) {
                        if let Ok(unblinded) = txout.unblind(&EC, blinding_key) {
                            let tx_ = if self.is_segwit() {
                                None
                            } else {
                                Some(tx.clone())
                            };
                            let external_utxo = ExternalUtxo {
                                outpoint,
                                txout: txout.clone(),
                                tx: tx_,
                                unblinded,
                                max_weight_to_satisfy: self.max_weight_to_satisfy(),
                            };
                            utxos.push(external_utxo);
                        }
                    }
                }
            }
        }
        Ok(utxos)
    }

    /// Attempt to unblind transaction outputs again
    ///
    /// In some quite particular situations, the wollet might have not unblinded some of
    /// its transaction outputs. This function allows to attempt to unblind them again.
    pub fn reunblind(&mut self) -> Result<Vec<OutPoint>, Error> {
        let mut txos = vec![];
        for (txid, tx) in self.store.cache.all_txs.iter() {
            for (vout, txout) in tx.output.iter().enumerate() {
                if self.store.cache.paths.contains_key(&txout.script_pubkey) {
                    let outpoint = OutPoint::new(*txid, vout as u32);
                    if let Entry::Vacant(e) = self.store.cache.unblinded.entry(outpoint) {
                        if let Ok(unblinded) = try_unblind(txout, &self.descriptor) {
                            e.insert(unblinded);
                            txos.push(outpoint);
                        }
                    }
                }
            }
        }
        Ok(txos)
    }

    pub(crate) fn balance_from_utxos(&self, utxos: &[WalletTxOut]) -> Result<Balance, Error> {
        let mut r = BTreeMap::new();
        r.entry(self.policy_asset()).or_insert(0);
        for u in utxos.iter() {
            *r.entry(u.unblinded.asset).or_default() += u.unblinded.value;
        }
        Ok(r.into())
    }

    /// Get the wallet balance
    pub fn balance(&self) -> Result<Balance, Error> {
        let utxos = self.utxos()?;
        self.balance_from_utxos(&utxos)
    }

    /// Get the asset identifiers owned by the wallet
    pub fn assets_owned(&self) -> Result<HashSet<AssetId>, Error> {
        let utxos = self.utxos()?;
        Ok(utxos.iter().map(|utxo| utxo.unblinded.asset).collect())
    }

    /// Get the wallet transactions with pagination
    pub fn transactions_paginated(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<WalletTx>, Error> {
        let mut txs = vec![];
        let mut my_txids: Vec<(&Txid, &Option<u32>)> = self.store.cache.heights.iter().collect();
        my_txids.sort_by(|a, b| {
            let height_cmp = b.1.unwrap_or(u32::MAX).cmp(&a.1.unwrap_or(u32::MAX));
            match height_cmp {
                Ordering::Equal => b.0.cmp(a.0),
                h => h,
            }
        });

        let txos = self.txos_map()?;
        for (txid, height) in my_txids.iter().skip(offset).take(limit) {
            let tx = self
                .store
                .cache
                .all_txs
                .get(*txid)
                .ok_or_else(|| Error::Generic(format!("list_tx no tx {txid}")))?;

            let balance = tx_balance(**txid, tx, &txos);
            if balance.is_empty() {
                // Transaction has no output or input that the wollet can unblind,
                // ignore this transaction
                continue;
            }
            let fee = tx_fee(tx);
            let policy_asset = self.policy_asset();
            let type_ = tx_type(tx, &policy_asset, &balance, fee);
            let timestamp = height.and_then(|h| self.store.cache.timestamps.get(&h).cloned());
            let inputs = tx_inputs(tx, &txos);
            let outputs = tx_outputs(**txid, tx, &txos);
            txs.push(WalletTx {
                tx: tx.clone(),
                txid: **txid,
                height: **height,
                balance: balance.into(),
                fee,
                type_,
                timestamp,
                inputs,
                outputs,
            });
        }

        Ok(txs)
    }

    /// Get the wallet transactions
    pub fn transactions(&self) -> Result<Vec<WalletTx>, Error> {
        self.transactions_paginated(0, usize::MAX)
    }

    /// Get a wallet transaction
    pub fn transaction(&self, txid: &Txid) -> Result<Option<WalletTx>, Error> {
        let height = self.store.cache.heights.get(txid);
        let tx = self.store.cache.all_txs.get(txid);
        if let (Some(height), Some(tx)) = (height, tx) {
            let txos = self.txos_map()?;

            let balance = tx_balance(*txid, tx, &txos);
            let fee = tx_fee(tx);
            let policy_asset = self.policy_asset();
            let type_ = tx_type(tx, &policy_asset, &balance, fee);
            let timestamp = height.and_then(|h| self.store.cache.timestamps.get(&h).cloned());
            let inputs = tx_inputs(tx, &txos);
            let outputs = tx_outputs(*txid, tx, &txos);

            Ok(Some(WalletTx {
                tx: tx.clone(),
                txid: *txid,
                height: *height,
                balance: balance.into(),
                fee,
                type_,
                timestamp,
                inputs,
                outputs,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get the wallet (re)issuances
    pub fn issuances(&self) -> Result<Vec<IssuanceDetails>, Error> {
        let mut r = vec![];
        for tx in self.transactions()? {
            r.extend(extract_issuances(&tx.tx));
        }
        Ok(r)
    }

    /// Get the issuance details for a certain asset
    ///
    /// This only works if the asset was issued by this wallet
    pub fn issuance(&self, asset: &AssetId) -> Result<IssuanceDetails, Error> {
        self.issuances()?
            .iter()
            .find(|d| &d.asset == asset && !d.is_reissuance)
            .cloned()
            .ok_or_else(|| Error::MissingIssuance)
    }

    /// Get the PSET details with respect to the wallet
    pub fn get_details(&self, pset: &PartiallySignedTransaction) -> Result<PsetDetails, Error> {
        Ok(PsetDetails {
            balance: pset_balance(pset, self.descriptor(), self.config.address_params())?,
            sig_details: pset_signatures(pset),
            issuances: pset_issuances(pset),
        })
    }

    pub(crate) fn index(&self, script_pubkey: &Script) -> Result<(Chain, u32), Error> {
        let (ext_int, index) = self
            .store
            .cache
            .paths
            .get(script_pubkey)
            .ok_or_else(|| Error::ScriptNotMine)?;
        let index = match index {
            ChildNumber::Normal { index } => index,
            ChildNumber::Hardened { index: _ } => {
                return Err(Error::Generic("unexpected hardened derivation".into()));
            }
        };
        Ok((*ext_int, *index))
    }

    // TODO: move to WolletDescriptor::definite_descriptor(index)
    pub(crate) fn definite_descriptor(
        &self,
        script_pubkey: &Script,
    ) -> Result<Descriptor<DefiniteDescriptorKey>, Error> {
        let (ext_int, utxo_index) = self.index(script_pubkey)?;
        self.descriptor.definite_descriptor(ext_int, utxo_index)
    }

    /// Add the PSET details with respect to the wallet
    pub fn add_details(&self, pset: &mut PartiallySignedTransaction) -> Result<(), Error> {
        let pset_clone = pset.clone();
        for (idx, input) in pset_clone.inputs().iter().enumerate() {
            if let Some(txout) = input.witness_utxo.as_ref() {
                match self.definite_descriptor(&txout.script_pubkey) {
                    Ok(desc) => {
                        pset.update_input_with_descriptor(idx, &desc)?;
                    }
                    Err(Error::ScriptNotMine) => (),
                    Err(e) => return Err(e),
                }
            }
        }

        for (idx, output) in pset_clone.outputs().iter().enumerate() {
            match self.definite_descriptor(&output.script_pubkey) {
                Ok(desc) => {
                    pset.update_output_with_descriptor(idx, &desc)?;
                }
                Err(Error::ScriptNotMine) => (),
                Err(e) => return Err(e),
            }
        }

        // Set PSET xpub origin
        self.descriptor().descriptor.for_each_key(|k| {
            match k {
                DescriptorPublicKey::XPub(x) => {
                    if let Some(origin) = &x.origin {
                        pset.global.xpub.insert(x.xkey, origin.clone());
                    }
                }
                DescriptorPublicKey::MultiXPub(x) => {
                    if let Some(origin) = &x.origin {
                        pset.global.xpub.insert(x.xkey, origin.clone());
                    }
                }
                _ => {}
            }
            true
        });

        Ok(())
    }

    /// Get the signers' fingerprints involved in this descriptor
    pub fn signers(&self) -> Vec<Fingerprint> {
        let mut signers = vec![];
        self.descriptor().descriptor.for_each_key(|k| {
            // xpub without key origin and single pubkey unexpectedly return a master fingerprint,
            // see tests below for the actual behaviour.
            // This should not be dangerous though, worst case is that we report a signer that
            // cannot sign.
            signers.push(k.master_fingerprint());
            true
        });
        signers
    }

    /// Combine a vector of PSET
    pub fn combine(
        &self,
        psets: &[PartiallySignedTransaction],
    ) -> Result<PartiallySignedTransaction, Error> {
        let mut res = psets.first().ok_or_else(|| Error::MissingPset)?.clone();
        for pset in psets.iter().skip(1) {
            res.merge(pset.clone())?;
        }
        Ok(res)
    }

    /// Finalize a PSET, extracting a broadcastable transaction
    pub fn finalize(&self, pset: &mut PartiallySignedTransaction) -> Result<Transaction, Error> {
        // elements-miniscript does not finalize PSET inputs if they have signature with different
        // sighashes. To workaround this, if necessary we replace the sighash in the signatures
        // with the sighash set in the PSET input. Then we finalize the PSET and later we replace
        // the sighashes with the original ones.
        let mut original_sigs = vec![];
        for i in pset.inputs_mut() {
            let sighash = i.sighash_type.map(|s| s.to_u32()).unwrap_or(1) as u8;
            if i.partial_sigs.len() > 1 {
                for sig in i.partial_sigs.values_mut() {
                    let sig_len = sig.len();
                    if sig_len > 0 && sig[sig_len - 1] != sighash {
                        original_sigs.push(sig.clone());
                        sig[sig_len - 1] = sighash;
                    }
                }
            }
        }

        // genesis_hash is only used for BIP341 (taproot) sighash computation
        let result = pset.finalize_mut(&EC, BlockHash::all_zeros());

        // Replace the original sighashes in the finalized signatures
        for original_sig in original_sigs {
            for i in pset.inputs_mut() {
                // TODO: also for pre-segwit inputs
                if let Some(witness) = &mut i.final_script_witness {
                    for e in witness.iter_mut() {
                        let len = original_sig.len();
                        if e.len() == len && e[0..(len - 1)] == original_sig[0..(len - 1)] {
                            *e = original_sig.clone();
                        }
                    }
                }
            }
        }

        if let Err(errors) = result {
            if !errors.is_empty() && errors.len() == pset.inputs().len() {
                // In some case "finalize" finalizes all inputs but return some error
                let seems_finalized = |i: &elements::pset::Input| -> bool {
                    i.partial_sigs.is_empty()
                        && (!i
                            .final_script_witness
                            .as_ref()
                            .is_some_and(|v| v.is_empty())
                            || !i.final_script_sig.as_ref().is_some_and(|v| v.is_empty()))
                };
                if !pset.inputs().iter().all(seems_finalized) {
                    // Failed to finalize all inputs
                    // TODO: do not use Generic
                    return Err(Error::Generic(format!("{errors:?}")));
                }
            }
            // If some inputs have been finalized ignore the other errors
        }

        Ok(pset.extract_tx()?)
    }

    /// Get all the persisted updates of this wallet.
    /// Applying in the same order these updates to an empty wallet, recreates this wallet state.
    pub fn updates(&self) -> Result<Vec<Update>, PersistError> {
        let mut updates = vec![];
        for i in 0.. {
            match self.persister.get(i)? {
                Some(update) => updates.push(update),
                None => break,
            }
        }
        Ok(updates)
    }

    /// A deterministic value derived from the descriptor, the config and the content of this wollet,
    /// including what's in the wallet store (transactions etc)
    ///
    /// In this case, we don't need cryptographic assurance guaranteed by the std default hasher (siphash)
    /// And we can use a much faster hasher, which is used also in the rust compiler.
    /// ([source](https://nnethercote.github.io/2021/12/08/a-brutally-effective-hash-function-in-rust.html))
    pub fn status(&self) -> u64 {
        let mut hasher = FxHasher::default();
        std::hash::Hash::hash(&self, &mut hasher);
        hasher.finish()
    }

    /// Returns true if this wollet has never received an updated applyed to it
    pub fn never_scanned(&self) -> bool {
        self.store.cache.tip == (0, BlockHash::all_zeros())
    }
}

fn tx_balance(
    txid: Txid,
    tx: &Transaction,
    txos: &HashMap<OutPoint, WalletTxOut>,
) -> BTreeMap<AssetId, i64> {
    debug_assert_eq!(txid, tx.txid());
    let mut balance = BTreeMap::new();

    for out_idx in 0..tx.output.len() {
        if let Some(txout) = txos.get(&OutPoint::new(txid, out_idx as u32)) {
            *balance.entry(txout.unblinded.asset).or_default() += txout.unblinded.value as i64;
        }
    }
    for input in &tx.input {
        if let Some(txout) = txos.get(&input.previous_output) {
            *balance.entry(txout.unblinded.asset).or_default() -= txout.unblinded.value as i64;
        }
    }
    balance
}

/// Performs a full blockchain scan using an Electrum client and applies any updates to the wallet.
///
/// For details about the scan see ['BlockchainBackend::full_scan']
#[cfg(feature = "electrum")]
pub fn full_scan_with_electrum_client(
    wollet: &mut Wollet,
    electrum_client: &mut crate::ElectrumClient,
) -> Result<(), Error> {
    full_scan_to_index_with_electrum_client(wollet, 0, electrum_client)
}

/// Like [`full_scan_with_electrum_client`] but scans up to a specific derivation index (see ['BlockchainBackend::full_scan_to_index'] for details)
#[cfg(feature = "electrum")]
pub fn full_scan_to_index_with_electrum_client(
    wollet: &mut Wollet,
    index: u32,
    electrum_client: &mut crate::ElectrumClient,
) -> Result<(), Error> {
    use crate::clients::blocking::BlockchainBackend;

    let update = electrum_client.full_scan_to_index(wollet, index)?;
    if let Some(update) = update {
        wollet.apply_update(update)?
    }

    Ok(())
}

fn tx_fee(tx: &Transaction) -> u64 {
    tx.output
        .iter()
        .filter(|o| o.script_pubkey.is_empty())
        .map(|o| o.value.explicit().unwrap_or(0))
        .sum()
}

/// Get a string that hopefully defines the transaction type.
///
/// Defining clear rules for types is highly arbitrary so here we provide a string that should
/// define the type, but it might be inaccurate in some cases.
fn tx_type(
    tx: &Transaction,
    policy_asset: &AssetId,
    balance: &BTreeMap<AssetId, i64>,
    fee: u64,
) -> String {
    let burn_script = burn_script();
    if tx
        .input
        .iter()
        .any(|i| !i.asset_issuance.is_null() && i.asset_issuance.asset_blinding_nonce == ZERO_TWEAK)
    {
        "issuance".to_string()
    } else if tx
        .input
        .iter()
        .any(|i| !i.asset_issuance.is_null() && i.asset_issuance.asset_blinding_nonce != ZERO_TWEAK)
    {
        "reissuance".to_string()
    } else if tx.output.iter().any(|o| o.script_pubkey == burn_script) {
        "burn".to_string()
    } else if balance.len() == 1 && balance.get(policy_asset) == Some(&(fee as i64)) {
        "redeposit".to_string()
    } else if balance.is_empty() {
        "unknown".to_string()
    } else if balance.values().all(|v| *v > 0) {
        "incoming".to_string()
    } else if balance.values().all(|v| *v < 0) {
        // redeposit case handled above
        "outgoing".to_string()
    } else {
        "unknown".to_string()
    }
}

fn tx_inputs(tx: &Transaction, txos: &HashMap<OutPoint, WalletTxOut>) -> Vec<Option<WalletTxOut>> {
    tx.input
        .iter()
        .map(|i| txos.get(&i.previous_output).cloned())
        .collect()
}

fn tx_outputs(
    txid: Txid, // passed to avoid expensive re-computation
    tx: &Transaction,
    txos: &HashMap<OutPoint, WalletTxOut>,
) -> Vec<Option<WalletTxOut>> {
    debug_assert_eq!(txid, tx.txid());

    (0..(tx.output.len() as u32))
        .map(|idx| txos.get(&OutPoint::new(txid, idx)).cloned())
        .collect()
}

/// Blockchain tip, the highest valid block in the blockchain
pub struct Tip {
    height: Height,
    hash: BlockHash,
    timestamp: Option<Timestamp>,
}

impl Tip {
    /// The height of the tip
    pub fn height(&self) -> Height {
        self.height
    }

    /// The hash of the block at the tip
    pub fn hash(&self) -> BlockHash {
        self.hash
    }

    /// The timestamp of the tip as unix timestamp (seconds since epoch)
    pub fn timestamp(&self) -> Option<Timestamp> {
        self.timestamp
    }
}

/// Derive script_pubkey and blinding_pubkey from a descriptor at a given derivation index
pub fn derive_script_and_blinding_key(
    descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    child: ChildNumber,
) -> Result<(Script, BlindingPublicKey), Error> {
    let address = descriptor
        .at_derivation_index(child.into())?
        .address(&EC, &AddressParams::ELEMENTS) // the params, doesn't matter, we don't use the address but its script and blinding pubkey, the latter cannot be retrieved in other way.
        .expect("all supported descriptors can generate an address");
    Ok((
        address.script_pubkey(),
        address
            .blinding_pubkey
            .expect("descriptor used include blinding key"),
    ))
}

#[cfg(feature = "test_wallet")]
impl Wollet {
    /// Create a new random test wallet with its signer.
    pub fn test_wallet() -> Result<(lwk_signer::SwSigner, Self), Error> {
        use lwk_common::Signer;
        use std::str::FromStr;

        let signer = lwk_signer::SwSigner::random(false)?.0;
        let desc = signer.wpkh_slip77_descriptor().map_err(Error::Generic)?;
        let desc = WolletDescriptor::from_str(&desc)?;
        Ok((
            signer,
            Wollet::new(ElementsNetwork::default_regtest(), NoPersist::new(), desc)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::str::FromStr;

    use super::*;
    use crate::elements::bitcoin::bip32::{Xpriv, Xpub};
    use crate::elements::bitcoin::network::Network;
    use crate::elements::AddressParams;
    use crate::{DownloadTxResult, NoPersist};
    use elements_miniscript::confidential::bare::tweak_private_key;
    use elements_miniscript::confidential::Key;
    use elements_miniscript::descriptor::checksum::desc_checksum;
    use elements_miniscript::descriptor::DescriptorSecretKey;
    use lwk_common::{singlesig_desc, DescriptorBlindingKey, Singlesig};
    use lwk_signer::SwSigner;

    #[test]
    fn test_desc() {
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let master_blinding_key =
            "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let checksum = "qw2qy2ml";
        let desc_str = format!("ct(slip77({master_blinding_key}),elwpkh({xpub}))#{checksum}");
        let desc = ConfidentialDescriptor::<DefiniteDescriptorKey>::from_str(&desc_str).unwrap();
        let addr = desc.address(&EC, &AddressParams::ELEMENTS).unwrap();
        let expected_addr = "el1qqthj9zn320epzlcgd07kktp5ae2xgx82fkm42qqxaqg80l0fszueszj4mdsceqqfpv24x0cmkvd8awux8agrc32m9nj9sp0hk";
        assert_eq!(addr.to_string(), expected_addr.to_string());
    }

    #[test]
    fn test_blinding_private() {
        // Get a confidential address from a "view" descriptor
        let seed = [0u8; 16];
        let xprv = Xpriv::new_master(Network::Regtest, &seed).unwrap();
        let xpub = Xpub::from_priv(&EC, &xprv);
        let checksum = "h0ej28gv";
        let desc_str = format!("ct({xprv},elwpkh({xpub}))#{checksum}");
        let desc = ConfidentialDescriptor::<DefiniteDescriptorKey>::from_str(&desc_str).unwrap();
        let address = desc.address(&EC, &AddressParams::ELEMENTS).unwrap();
        // and extract the public blinding key
        let pk_from_addr = address.blinding_pubkey.unwrap();

        // Get the public blinding key from the descriptor blinding key
        let key = match desc.key {
            Key::View(DescriptorSecretKey::XPrv(dxk)) => dxk.xkey.to_priv(),
            _ => todo!(),
        };
        let tweaked_key = tweak_private_key(&EC, &address.script_pubkey(), &key.inner);
        let pk_from_view = tweaked_key.public_key(&EC);

        assert_eq!(pk_from_addr, pk_from_view);
    }

    #[test]
    fn test_view_single() {
        let descriptor_blinding_key =
            "1111111111111111111111111111111111111111111111111111111111111111";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let desc_str = format!("ct({descriptor_blinding_key},elwpkh({xpub}))");
        let desc_str = format!("{desc_str}#{}", desc_checksum(&desc_str).unwrap());
        let _desc = ConfidentialDescriptor::<DefiniteDescriptorKey>::from_str(&desc_str).unwrap();
    }

    fn new_wollet(desc: &str) -> Wollet {
        let desc: WolletDescriptor = format!("{desc}#{}", desc_checksum(desc).unwrap())
            .parse()
            .unwrap();
        Wollet::new(ElementsNetwork::LiquidTestnet, NoPersist::new(), desc).unwrap()
    }

    #[test]
    fn test_signers() {
        let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";

        let fp1 = Fingerprint::from_str("11111111").unwrap();
        let fp2 = Fingerprint::from_str("22222222").unwrap();
        let fp_xpub = Fingerprint::from_str("0a55db61").unwrap();
        let fp_single = Fingerprint::from_str("51814f10").unwrap();

        let signer1 = format!("[{fp1}/0h/0h/0h]{xpub}/0/*");
        let signer1_mp = format!("[{fp1}/0h/0h/0h]{xpub}/<0;1>/*");
        let signer2 = format!("[{fp2}/0h/0h/0h]{xpub}/0/*");
        let signer_xpub = format!("{xpub}/0/*"); // no keyorigin
        let signer_single = "020202020202020202020202020202020202020202020202020202020202020202";

        let desc_s_1 = format!("ct({view_key},elwpkh({signer1}))");
        let desc_s_1mp = format!("ct({view_key},elwpkh({signer1_mp}))");
        let desc_s_xpub = format!("ct({view_key},elwpkh({signer_xpub}))");
        let desc_m_1single = format!("ct({view_key},elwsh(multi(2,{signer1},{signer_single})))");
        let desc_m_12 = format!("ct({view_key},elwsh(multi(2,{signer1},{signer2})))");

        assert_eq!(new_wollet(&desc_s_1).signers(), vec![fp1]);
        assert_eq!(new_wollet(&desc_s_1mp).signers(), vec![fp1]);
        assert_eq!(new_wollet(&desc_s_xpub).signers(), vec![fp_xpub]);
        assert_eq!(new_wollet(&desc_m_1single).signers(), vec![fp1, fp_single]);
        assert_eq!(new_wollet(&desc_m_12).signers(), vec![fp1, fp2]);
    }

    #[test]
    fn test_restore_only_tip() {
        let desc_str = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))";
        let desc: WolletDescriptor = format!("{desc_str}#{}", desc_checksum(desc_str).unwrap())
            .parse()
            .unwrap();

        let tempdir = tempfile::tempdir().unwrap();
        let persister = FsPersister::new(&tempdir, ElementsNetwork::LiquidTestnet, &desc).unwrap();
        let mut wollet = Wollet::new(
            ElementsNetwork::LiquidTestnet,
            persister.clone(),
            desc.clone(),
        )
        .unwrap();

        let tip = lwk_test_util::liquid_block_1().header;
        let update = Update {
            version: 1,
            wollet_status: wollet.status(),
            new_txs: DownloadTxResult::default(),
            txid_height_new: Vec::new(),
            txid_height_delete: Vec::new(),
            timestamps: vec![(tip.height, tip.time)],
            scripts_with_blinding_pubkey: Vec::new(),
            tip,
        };

        wollet.apply_update(update.clone()).unwrap();

        // Apply second only tip update with different block timestamps
        let new_tip = lwk_test_util::liquid_block_header_2_963_520();
        let update2 = Update {
            tip: new_tip.clone(),
            timestamps: vec![(new_tip.height, new_tip.time)],
            wollet_status: wollet.status(),
            ..update.clone()
        };
        wollet.apply_update(update2).unwrap();

        // We restore the wallet and expects the same status
        let restored_wollet =
            Wollet::new(ElementsNetwork::LiquidTestnet, persister.clone(), desc).unwrap();

        assert_eq!(wollet.status(), restored_wollet.status());
    }

    #[test]
    fn test_apply_old_update() {
        let bytes = lwk_test_util::update_test_vector_bytes();

        let update_1 = crate::Update::deserialize(&bytes[..]).unwrap();
        assert_eq!(update_1.tip.height, 1);
        let mut update_3 = update_1.clone();
        update_3.tip.height = 3;
        let mut update_4 = update_1.clone();
        update_4.tip.height = 4;

        let exp = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))";
        let mut wollet = new_wollet(exp);
        wollet.apply_update(update_4).unwrap();
        wollet.apply_update(update_3).unwrap(); // 1 block behing it's ok, maximum possible reorg on liquid
        let err = wollet.apply_update(update_1).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Update height 1 too old (internal height 3)"
        );
    }

    #[test]
    fn test_desc_no_wildcard_with_index() {
        let k = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let x = "tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M";
        let desc = format!("ct(slip77({k}),elwpkh({x}))");
        let w = new_wollet(&desc);
        let a = w.address(None).unwrap();
        assert_eq!(a.address().to_string(), "tlq1qqtkq6nptvwfycgsvkclsg8uyslwy9pn5mmw6049nmqq02y7l9330a6vmsc5zdfq2xtpyc7tct5rtr80rlvrk7jll6mc5gjfup");
        let e = w.address(Some(0)).unwrap_err();
        assert_eq!(
            e.to_string(),
            "Cannot use derivation index when the descriptor has no wildcard"
        );
    }

    #[test]
    fn fixed_addresses_test() {
        let expected = [
            "lq1qqvxk052kf3qtkxmrakx50a9gc3smqad2ync54hzntjt980kfej9kkfe0247rp5h4yzmdftsahhw64uy8pzfe7cpg4fgykm7cv", //  network: Liquid variant: Wpkh blinding_variant: Slip77
            "lq1qqtmf5e3g4ats3yexwdfn6kfhp9sl68kdl47g75k58rvw2w33zuarwfe0247rp5h4yzmdftsahhw64uy8pzfe7k9s63c7cku58", // network: Liquid variant: Wpkh blinding_variant: Elip151
            "VJLCQwwG8s7qUGhpJkQpkf7wLoK785TcK2cPqka8675FeJB7NEHLto5MUJyhJURGJCbFHA6sb6rgTwbh", // network: Liquid variant: ShWpkh blinding_variant: Slip77
            "VJLD3sfRNBrKyQkJp9KpLqSVtD9YWswXctqzdFhsctaDCwLoUcSato1DfspVSGMbk28avytesWFhiv37", // network: Liquid variant: ShWpkh blinding_variant: Elip151
            "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn", // network: LiquidTestnet variant: Wpkh blinding_variant: Slip77
            "tlq1qqv74shw44vxlpdhtmwqc2zfr5365hm8p6rg8cjnu77w57000dmuc05xy50hsn6vhkm5euwt72x878eq6zxx2z4zm4jus26k72", // network: LiquidTestnet variant: Wpkh blinding_variant: Elip151 i:5
            "vjTwLVioiKrDJ7zZZn9iQQrxP6RPpcvpHBhzZrbdZKKVZE29FuXSnkXdKcxK3qD5t1rYsdxcm9KYRMji", // network: LiquidTestnet variant: ShWpkh blinding_variant: Slip77
            "vjU3guCqyPrnKFXsUhpKPhUyduT6Zjr3b2ukPhE9BpiW4LpehTRvw4FHKxkMw7TRAzE7KhtsnkZ4rPth", // network: LiquidTestnet variant: ShWpkh blinding_variant: Elip151 i:7
            "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq", // network: ElementsRegtest { policy_asset: 0000000000000000000000000000000000000000000000000000000000000000 } variant: Wpkh blinding_variant: Slip77
            "el1qqv74shw44vxlpdhtmwqc2zfr5365hm8p6rg8cjnu77w57000dmuc05xy50hsn6vhkm5euwt72x878eq6zxx2zw8kxk9q8g9ne", // network: ElementsRegtest { policy_asset: 0000000000000000000000000000000000000000000000000000000000000000 } variant: Wpkh blinding_variant: Elip151 i:9
            "AzpmUtw4GMrEsfz6GKx5SKT1DV3qLS3xtSGdKG351rMjGxoUwS6Vsbu3zu2opBiPtjWs1GnE48uMFFnb", // network: ElementsRegtest { policy_asset: 0000000000000000000000000000000000000000000000000000000000000000 } variant: ShWpkh blinding_variant: Slip77
            "AzpsqJR6XRrotoXQBFcgRc52UJ5Y5YyCCHUP96faeMkjn5bzNyzz1uci1EprhTxjBhtRTLiV5k6sWP7j", // network: ElementsRegtest { policy_asset: 0000000000000000000000000000000000000000000000000000000000000000 } variant: ShWpkh blinding_variant: Elip151 i:11
            ];
        let mut i = 0usize;
        let mnemonic = lwk_test_util::TEST_MNEMONIC;

        for network in [
            ElementsNetwork::Liquid,
            ElementsNetwork::LiquidTestnet,
            ElementsNetwork::ElementsRegtest {
                policy_asset: AssetId::default(),
            },
        ] {
            let is_mainnet = matches!(network, ElementsNetwork::Liquid);
            let signer = SwSigner::new(mnemonic, is_mainnet).unwrap();
            for script_variant in [Singlesig::Wpkh, Singlesig::ShWpkh] {
                for blinding_variant in [
                    DescriptorBlindingKey::Slip77,
                    DescriptorBlindingKey::Elip151,
                ] {
                    let desc: WolletDescriptor =
                        singlesig_desc(&signer, script_variant, blinding_variant)
                            .unwrap()
                            .parse()
                            .unwrap();

                    let wollet = Wollet::new(network, NoPersist::new(), desc).unwrap();
                    let first_address = wollet.address(Some(0)).unwrap();
                    assert_eq!(first_address.address().to_string(), expected[i], "network: {network:?} variant: {script_variant:?} blinding_variant: {blinding_variant:?} i:{i}");
                    i += 1;
                }
            }
        }
    }

    #[test]
    fn test_wollet_status() {
        let bytes = lwk_test_util::update_test_vector_bytes();

        let update = crate::Update::deserialize(&bytes[..]).unwrap();
        let exp = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))";
        let mut wollet = new_wollet(exp);

        let mut hasher = DefaultHasher::new();
        wollet.hash(&mut hasher);
        assert_eq!(12092173119280468224, hasher.finish());

        assert!(wollet.never_scanned());
        wollet.apply_update(update).unwrap();
        assert!(!wollet.never_scanned());

        let mut hasher = FxHasher::default();
        wollet.hash(&mut hasher);
        assert_eq!(4667218140179748739, hasher.finish());

        assert_eq!(4667218140179748739, wollet.status());
    }

    #[test]
    fn test_wollet_pegin_address() {
        let fed_desc: BtcDescriptor<bitcoin::PublicKey> =
            BtcDescriptor::<bitcoin::PublicKey>::from_str(lwk_test_util::FED_PEG_DESC).unwrap();
        let wollet = new_wollet(lwk_test_util::PEGIN_TEST_DESC);
        let addr = wollet.pegin_address(Some(0), fed_desc).unwrap();
        assert_eq!(addr.tweak_index(), 0);
        assert_eq!(addr.address().to_string(), lwk_test_util::PEGIN_TEST_ADDR);
    }

    #[test]
    fn test_txos_inner() {
        let wollet = test_wollet_with_many_transactions();
        let utxos = wollet.utxos_inner().unwrap();
        assert_eq!(utxos.len(), 26);
        let txos = wollet.txos_inner().unwrap();
        assert_eq!(txos.len(), 132);
    }

    #[test]
    fn test_acceptable_performance() {
        let wollet = test_wollet_with_many_transactions();

        // This constant represents the maximum acceptable duration for the tested methods.
        // The value needs to account for tests running in debug mode, which is much slower
        // than release mode where benchmarks show these methods take less than 1ms.
        // Also, CI is slow, so we need to account for that.
        // We chose 150ms to catch significant performance regressions.
        const MAX_DURATION: std::time::Duration = std::time::Duration::from_millis(150);

        let start = std::time::Instant::now();
        let _txs = wollet.transactions().unwrap();
        let duration = start.elapsed();
        println!("duration: {duration:?}");
        assert!(duration < MAX_DURATION);

        let start = std::time::Instant::now();
        let _utxos = wollet.utxos().unwrap();
        let duration = start.elapsed();
        assert!(duration < MAX_DURATION);

        let start = std::time::Instant::now();
        let _txos = wollet.txos().unwrap();
        let duration = start.elapsed();
        assert!(duration < MAX_DURATION);
    }

    #[test]
    fn test_transactions_pagination() {
        let wollet = test_wollet_with_many_transactions();

        // Get all transactions
        let all_txs = wollet.transactions().unwrap();
        let total_txs = all_txs.len();
        assert!(total_txs > 0, "Test wallet should have some transactions");

        // Test pagination with different offsets and limits
        let test_cases = vec![
            (0, 5),             // First 5 transactions
            (5, 5),             // Next 5 transactions
            (10, 10),           // Next 10 transactions
            (0, 1),             // Just the first transaction
            (total_txs - 1, 1), // Just the last transaction
            (total_txs, 1),     // Offset beyond available transactions
            (0, total_txs + 1), // Limit larger than available transactions
        ];

        for (offset, limit) in test_cases {
            let paginated_txs = wollet.transactions_paginated(offset, limit).unwrap();

            // Verify the number of returned transactions
            let expected_count = if offset >= total_txs {
                0
            } else {
                std::cmp::min(limit, total_txs - offset)
            };
            assert_eq!(
                paginated_txs.len(),
                expected_count,
                "Wrong number of transactions for offset={offset}, limit={limit}",
            );

            // Verify the transactions match the expected slice of all transactions
            if !paginated_txs.is_empty() {
                let expected_txs = &all_txs[offset..offset + paginated_txs.len()];
                assert_eq!(
                    paginated_txs, expected_txs,
                    "Transactions don't match for offset={offset}, limit={limit}",
                );
            }
        }
    }

    // duplicated from tests/test_wollet.rs
    pub fn test_wollet_with_many_transactions() -> Wollet {
        let update = lwk_test_util::update_test_vector_many_transactions();
        let descriptor = lwk_test_util::wollet_descriptor_many_transactions();
        let descriptor: WolletDescriptor = descriptor.parse().unwrap();
        let update = Update::deserialize(&update).unwrap();
        let network = ElementsNetwork::LiquidTestnet;
        let mut wollet = WolletBuilder::new(network, descriptor).build().unwrap();
        wollet.apply_update(update).unwrap();
        wollet
    }
}
