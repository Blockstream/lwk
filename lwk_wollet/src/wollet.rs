use crate::bitcoin::bip32::Fingerprint;
use crate::cache::{Cache, Height, ScriptBatch, Timestamp, BATCH_SIZE};
use crate::clients::{try_unblind, LastUnused};
use crate::descriptor::Chain;
use crate::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use crate::elements::pset::PartiallySignedTransaction;
use crate::elements::secp256k1_zkp::ZERO_TWEAK;
use crate::elements::{AssetId, BlockHash, OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::error::Error;
use crate::hashes::{sha256t_hash_newtype, Hash};
use crate::model::{AddressResult, ExternalUtxo, IssuanceDetails, WalletTx, WalletTxOut};
use crate::pegin::{FedPeg, PeginAddress};
use crate::tx_builder::{extract_issuances, WolletTxBuilder};
use crate::update::UpdatesPersister;
use crate::util::EC;
use crate::Network;
use crate::{BlindingPublicKey, WolletDescriptor};
use elements::bitcoin::bip32::ChildNumber;
use elements::{bitcoin, Address};
use elements_miniscript::psbt::PsbtExt;
use elements_miniscript::ForEachKey;
use elements_miniscript::{
    ConfidentialDescriptor, DefiniteDescriptorKey, Descriptor, DescriptorPublicKey,
};
use fxhash::FxHasher;
use lwk_common::{
    burn_script, get_genesis_hash, pset_balance, pset_issuances, pset_signatures, Balance,
    DynStore, EncryptedStore, FakeStore, FileStore, MemoryStore, PsetDetails,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::Hasher;
use std::path::Path;
use std::sync::{atomic, Arc};

sha256t_hash_newtype! {
    /// The tag of the hash
    pub struct DirectoryIdTag = hash_str("LWK-FS-Directory-Id/1.0");

    /// A tagged hash to generate the name of the subdirectory to store cache content
    #[hash_newtype(forward)]
    pub struct DirectoryIdHash(_);
}

/// A watch-only wallet defined by a CT descriptor.
pub struct Wollet {
    pub(crate) network: Network,
    pub(crate) cache: Cache,
    pub(crate) descriptor: WolletDescriptor,

    // Fields for managing `Update`s
    pub(crate) updates_persister: UpdatesPersister,

    /// cached value
    max_weight_to_satisfy: usize,
    utxo_only: bool,

    /// Silent-payment scan-only material.
    #[cfg(feature = "silentpayments")]
    pub(crate) silent_payment_material: Option<crate::silentpayments::SilentPaymentScanMaterial>,

    /// Where discovery starts when it has never run. See
    /// [`WolletBuilder::with_silent_payment_birthday()`].
    ///
    /// Not persisted: it describes how to *begin* scanning, and once a scan has run the
    /// recorded progress supersedes it. Persisting it would create two sources of truth
    /// for the resume point.
    #[cfg(feature = "silentpayments")]
    pub(crate) silent_payment_birthday: Option<Height>,
}

/// A builder for constructing [`Wollet`] instances
#[derive(Debug)]
pub struct WolletBuilder {
    network: Network,
    descriptor: WolletDescriptor,
    with_stores_disallowed: bool,
    updates_store: Arc<dyn DynStore>,
    /// Number of updates to trigger merge. None disables merging.
    merge_threshold: Option<usize>,
    encrypt_updates_store: Option<bool>,
    txs_store: Arc<dyn DynStore>,
    encrypt_txs_store: Option<bool>,
    utxo_only: bool,
    #[cfg(feature = "silentpayments")]
    silent_payment_material: Option<crate::silentpayments::SilentPaymentScanMaterial>,
    #[cfg(feature = "silentpayments")]
    silent_payment_birthday: Option<Height>,
}

impl WolletBuilder {
    /// Create a `Wollet` builder
    pub fn new(network: Network, descriptor: WolletDescriptor) -> Self {
        Self {
            network,
            descriptor,
            with_stores_disallowed: false,
            updates_store: Arc::new(FakeStore::new()),
            encrypt_updates_store: None,
            txs_store: Arc::new(MemoryStore::new()),
            encrypt_txs_store: None,
            merge_threshold: None,
            utxo_only: false,
            #[cfg(feature = "silentpayments")]
            silent_payment_material: None,
            #[cfg(feature = "silentpayments")]
            silent_payment_birthday: None,
        }
    }

    /// Sets scan-only material for receiving silent payments.
    #[cfg(feature = "silentpayments")]
    pub fn with_silent_payment_material(
        mut self,
        material: crate::silentpayments::SilentPaymentScanMaterial,
    ) -> Self {
        self.silent_payment_material = Some(material);
        self
    }

    /// Sets the first block used for silent-payment discovery.
    #[cfg(feature = "silentpayments")]
    pub fn with_silent_payment_birthday(mut self, height: Height) -> Self {
        self.silent_payment_birthday = Some(height);
        self
    }

    /// Set all key-value stores for peristence
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// Use `store` for all key-value stores used by the `Wollet`.
    pub fn with_stores(mut self, store: Arc<dyn DynStore>) -> Result<Self, Error> {
        if self.with_stores_disallowed {
            return Err(Error::Generic(
                "`with_stores` must be called before other functions".into(),
            ));
        }
        self.updates_store = store.clone();
        self.txs_store = store;
        self.merge_threshold = Some(1);
        Ok(self)
    }

    /// Set a threshold to merge updates
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// When the number of updates exceeds the threshold, they are merged into one.
    /// Set to None to disable merging (default).
    pub fn with_merge_threshold(mut self, threshold: Option<usize>) -> Self {
        self.merge_threshold = threshold;
        self.with_stores_disallowed = true;
        self
    }

    /// Allow the `Wollet` to work with `utxo_only` clients
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// [`crate::clients::WaterfallsClientBuilder`] allows to create clients that only fetch transactions with
    /// unspent outputs to minimize the network and memory usage.
    ///
    /// If `utxo_only` is `true`, the `Wollet` will only work with `utxo_only` clients.
    /// If `utxo_only` is `false` (default), the `Wollet` will only work with non-`utxo_only` clients.
    pub fn utxo_only(mut self, utxo_only: bool) -> Self {
        self.utxo_only = utxo_only;
        self
    }

    /// Specify the store used to persist `Update`s
    ///
    /// **Experimental**: this API might change without notice.
    pub fn with_updates_store(mut self, store: Arc<dyn DynStore>) -> Self {
        self.updates_store = store;
        self.with_stores_disallowed = true;
        self
    }

    /// Specify the `Wollet` store for persistence
    #[deprecated(
        since = "0.17.0",
        note = "please use `with_stores` or `with_updates_store`"
    )]
    pub fn with_store(mut self, store: Arc<dyn DynStore>) -> Self {
        self.updates_store = store;
        self.with_stores_disallowed = true;
        self
    }

    /// Specify the store used to persist wallet transactions
    ///
    /// **Experimental**: this API might change without notice.
    pub fn with_txs_store(mut self, store: Arc<dyn DynStore>) -> Self {
        self.txs_store = store;
        self.with_stores_disallowed = true;
        self
    }

    /// Set encryption for txs store
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// Default: encrypted if store is persisted
    pub fn set_encryption_txs_store(mut self, encrypt: bool) -> Self {
        self.encrypt_txs_store = Some(encrypt);
        self
    }

    /// Set encryption for updates store
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// Default: encrypted if store is persisted
    pub fn set_encryption_updates_store(mut self, encrypt: bool) -> Self {
        self.encrypt_updates_store = Some(encrypt);
        self
    }

    /// Create the legacy `Wollet` (unencrypted) updates store
    pub fn create_legacy_fs_store<P: AsRef<Path>>(&self, datadir: P) -> Result<FileStore, Error> {
        // Build path: datadir/network/enc_cache/hash(descriptor)
        let mut path = datadir.as_ref().to_path_buf();
        path.push(self.network.as_str());
        path.push("enc_cache");
        path.push(DirectoryIdHash::hash(self.descriptor.to_string().as_bytes()).to_string());

        let file_store = FileStore::new(path)?;
        Ok(file_store)
    }

    /// Use the legacy `Wollet` store
    pub fn with_legacy_fs_store<P: AsRef<Path>>(mut self, datadir: P) -> Result<Self, Error> {
        let file_store = self.create_legacy_fs_store(datadir)?;
        self.updates_store = Arc::new(file_store);
        self.with_stores_disallowed = true;
        Ok(self)
    }

    /// Build the `Wollet`
    pub fn build(self) -> Result<Wollet, Error> {
        if self.txs_store.is_persisted() && self.merge_threshold != Some(1) {
            // This simplifies implementation, otherwise we would need to remember in which
            // update each transaction was added.
            return Err(Error::Generic(
                "If txs store is persited, merge threshold must be 1".into(),
            ));
        }

        let encrypt_txs_store = self
            .encrypt_txs_store
            .unwrap_or(self.txs_store.is_persisted());
        let txs_store = if encrypt_txs_store {
            let key_bytes = self.descriptor.encryption_key_bytes();
            let encrypted_store =
                EncryptedStore::new_with_key_encryption(self.txs_store, key_bytes);
            Arc::new(encrypted_store)
        } else {
            self.txs_store
        };

        let encrypt_updates_store = self
            .encrypt_updates_store
            .unwrap_or(self.updates_store.is_persisted());
        let updates_store = if encrypt_updates_store {
            let key_bytes = self.descriptor.encryption_key_bytes();
            // For updates, keys are not encrypted
            let encrypted_store = EncryptedStore::new(self.updates_store, key_bytes);
            Arc::new(encrypted_store)
        } else {
            self.updates_store
        };

        let cache = Cache::new(txs_store);
        let max_weight_to_satisfy = match self.descriptor.definite_descriptor(Chain::External, 0) {
            Ok(d) => d.max_weight_to_satisfy()?,
            Err(_) => 0, // If we don't have the descriptor we don't know this value
        };
        let updates_persister = UpdatesPersister::new(updates_store, self.merge_threshold);
        let mut wollet = Wollet {
            cache,
            network: self.network,
            descriptor: self.descriptor,
            updates_persister,
            max_weight_to_satisfy,
            utxo_only: self.utxo_only,
            #[cfg(feature = "silentpayments")]
            silent_payment_material: self.silent_payment_material,
            #[cfg(feature = "silentpayments")]
            silent_payment_birthday: self.silent_payment_birthday,
        };

        wollet.restore_updates()?;
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
    scripts: HashMap<(Chain, ChildNumber), (Script, Option<BlindingPublicKey>)>,
    heights: HashMap<Txid, Option<Height>>,
    tip: (Height, BlockHash),
    last_unused: LastUnused,
    utxo_only: bool,
}

pub trait WolletState {
    fn get_script_batch(&self, batch: u32, ext_int: Chain) -> Result<ScriptBatch, Error>;
    fn get_or_derive(
        &self,
        ext_int: Chain,
        child: ChildNumber,
    ) -> Result<(Script, Option<BlindingPublicKey>, bool), Error>;
    fn heights(&self) -> &HashMap<Txid, Option<Height>>;
    fn paths(&self) -> &HashMap<Script, (Chain, ChildNumber)>;
    fn txs(&self) -> HashSet<Txid>;
    fn tip(&self) -> (Height, BlockHash);
    fn last_unused(&self) -> LastUnused; // TODO change to &LastUnused when possible
    fn descriptor(&self) -> WolletDescriptor;
    fn wollet_status(&self) -> u64;
    fn utxo_only(&self) -> bool;

    /// The silent payment scan-only material, if this wallet has any.
    ///
    /// A scan needs this to discover silent payments, and it cannot be recovered
    /// from the descriptor — detection requires `b_scan`, which a CT descriptor cannot
    /// express. `None` means the wallet can send silent payments but not receive them,
    /// and a scan skips discovery entirely.
    #[cfg(feature = "silentpayments")]
    fn silent_payment_material(&self) -> Option<&crate::silentpayments::SilentPaymentScanMaterial> {
        None
    }

    /// The height through which silent payments have already been discovered.
    ///
    /// Discovery cannot resume from the descriptor-scan tip: the two advance
    /// independently, and a silent payment scan that restarted from the wallet tip after
    /// every restart would re-download every block in between.
    #[cfg(feature = "silentpayments")]
    fn silent_payments_scanned_to(&self) -> Option<Height> {
        None
    }

    /// The height at which discovery should begin when it has never run.
    ///
    /// See [`WolletBuilder::with_silent_payment_birthday()`]. `None` means genesis,
    /// which is correct but only affordable on a short chain.
    #[cfg(feature = "silentpayments")]
    fn silent_payment_birthday(&self) -> Option<Height> {
        None
    }

    /// The first height a scan must examine: one past whatever discovery already
    /// covered, or the birthday if it has never run.
    ///
    /// Provided by the trait so every backend resolves this the same way. Getting it
    /// wrong in either direction is costly — too low re-downloads the chain, too high
    /// silently skips payments — so it is defined once rather than at each call site.
    #[cfg(feature = "silentpayments")]
    fn silent_payments_scan_from(&self) -> Height {
        match self.silent_payments_scanned_to() {
            // Recorded progress always wins: it is a fact about what was scanned,
            // whereas the birthday is only a hint about where to start.
            Some(scanned_to) => scanned_to.saturating_add(1),
            None => self.silent_payment_birthday().unwrap_or(0),
        }
    }
}

impl WolletState for WolletConciseState {
    // TODO duplicated from Wollet
    fn get_script_batch(&self, batch: u32, ext_int: Chain) -> Result<ScriptBatch, Error> {
        let descriptor = &self.descriptor;
        let mut result = ScriptBatch {
            cached: true,
            ..Default::default()
        };

        // For Spks, return all scripts in batch 0, empty for subsequent batches
        if let Some(count) = descriptor.spk_count() {
            if batch > 0 {
                return Ok(result);
            }
            for j in 0..count as u32 {
                let child = ChildNumber::from_normal_idx(j)?;
                let (script, blinding_pubkey, cached) =
                    self.get_or_derive(Chain::External, child)?;
                result.cached = cached;
                result
                    .value
                    .push((script, (Chain::External, child, blinding_pubkey)));
            }
            return Ok(result);
        }

        let start = batch * BATCH_SIZE;
        let end = start + BATCH_SIZE;
        for j in start..end {
            let child = ChildNumber::from_normal_idx(j)?;
            let (script, blinding_pubkey, cached) = self.get_or_derive(ext_int, child)?;
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
    ) -> Result<(Script, Option<BlindingPublicKey>, bool), Error> {
        let opt_script = self.scripts.get(&(ext_int, child));
        let (script, blinding_pubkey, cached) = match opt_script {
            Some((script, blinding_pubkey)) => (script.clone(), *blinding_pubkey, true),
            None => {
                let (script, blinding_pubkey) = self
                    .descriptor
                    .derive_script_and_blinding_key(ext_int, child)?;
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

    fn utxo_only(&self) -> bool {
        self.utxo_only
    }
}

impl std::fmt::Debug for Wollet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "wollet({:?})", self.descriptor)
    }
}

impl WolletState for Wollet {
    fn get_script_batch(&self, batch: u32, ext_int: Chain) -> Result<ScriptBatch, Error> {
        self.cache
            .get_script_batch(batch, &self.descriptor, ext_int)
    }

    fn get_or_derive(
        &self,
        ext_int: Chain,
        child: ChildNumber,
    ) -> Result<(Script, Option<BlindingPublicKey>, bool), Error> {
        self.cache.get_or_derive(ext_int, child, &self.descriptor)
    }

    fn heights(&self) -> &HashMap<Txid, Option<Height>> {
        self.cache.heights()
    }

    fn paths(&self) -> &HashMap<Script, (Chain, ChildNumber)> {
        &self.cache.paths
    }

    fn txs(&self) -> HashSet<Txid> {
        self.cache.all_txids().clone()
    }

    fn tip(&self) -> (Height, BlockHash) {
        self.cache.tip
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

    fn utxo_only(&self) -> bool {
        self.utxo_only
    }

    #[cfg(feature = "silentpayments")]
    fn silent_payment_material(&self) -> Option<&crate::silentpayments::SilentPaymentScanMaterial> {
        self.silent_payment_material.as_ref()
    }

    #[cfg(feature = "silentpayments")]
    fn silent_payments_scanned_to(&self) -> Option<Height> {
        self.cache.silent_payments_scanned_to
    }

    #[cfg(feature = "silentpayments")]
    fn silent_payment_birthday(&self) -> Option<Height> {
        self.silent_payment_birthday
    }
}

impl std::hash::Hash for Wollet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.network.hash(state);
        self.cache.hash(state);
        self.descriptor.hash(state);
        #[cfg(feature = "silentpayments")]
        self.hash_silent_payment_identity(state);
    }
}

/// The silent-payment half of a wallet's persistence identity.
///
/// Only compiled with the feature: without it a wallet has no scan material, so
/// there is nothing to contribute and no call site to gate.
#[cfg(feature = "silentpayments")]
impl Wollet {
    /// Domain tag for the silent-payment section of [`Wollet::status()`].
    ///
    /// Version for the silent-payment identity hash.
    const STATUS_DOMAIN: &'static [u8] = b"lwk/silentpayments/status/v1";

    /// Mix this wallet's silent-payment identity into its `status()` hash.
    ///
    /// Hashes the presence and identity of scan material.
    ///
    /// The layout is `domain tag ‖ present flag ‖ coin_type ‖ account ‖ B_scan ‖ B_spend`.
    ///
    /// The section is omitted when no material exists, preserving the legacy hash.
    /// Account coordinates and both public points identify the material; the secret
    /// scan key is never hashed.
    fn hash_silent_payment_identity<H: std::hash::Hasher>(&self, state: &mut H) {
        use std::hash::Hash;

        match self.silent_payment_material.as_ref() {
            None => {}
            Some(material) => {
                Self::STATUS_DOMAIN.hash(state);
                true.hash(state);
                let account = material.account();
                account.coin_type().hash(state);
                account.account().hash(state);
                material
                    .scan_pubkey(&crate::util::EC)
                    .serialize()
                    .hash(state);
                material.spend_pubkey().serialize().hash(state);
            }
        }
    }
}

impl Wollet {
    /// Create a new wallet
    pub fn new(
        network: Network,
        store: Arc<dyn DynStore>,
        descriptor: WolletDescriptor,
    ) -> Result<Self, Error> {
        WolletBuilder::new(network, descriptor)
            .with_updates_store(store)
            .build()
    }

    /// Whether the wallet is segwit (BIP141)
    pub fn is_segwit(&self) -> bool {
        self.descriptor()
            .is_ok_and(|d| d.descriptor.desc_type().segwit_version().is_some())
    }

    /// Whether the wallet is AMP0
    #[cfg(feature = "amp0")]
    pub fn is_amp0(&self) -> bool {
        self.descriptor.is_amp0()
    }

    /// Whether the wallet is `utxo_only`
    pub fn utxo_only(&self) -> bool {
        self.utxo_only
    }

    /// Max weight to satisfy for inputs belonging to this wallet
    pub fn max_weight_to_satisfy(&self) -> usize {
        self.max_weight_to_satisfy
    }

    /// Get a concise state of the wallet, allowing to perform a scan (like [`crate::clients::blocking::BlockchainBackend::full_scan()`]) without holding the lock on the wallet.
    pub fn state(&self) -> WolletConciseState {
        let cache = &self.cache;
        WolletConciseState {
            wollet_status: self.status(),
            descriptor: self.wollet_descriptor(),
            txs: cache.all_txids().clone(),
            paths: cache.paths.clone(),
            scripts: cache.scripts.clone(),
            heights: cache.heights().clone(),
            tip: cache.tip,
            last_unused: LastUnused {
                internal: cache.last_unused_internal.load(atomic::Ordering::Relaxed),
                external: cache.last_unused_external.load(atomic::Ordering::Relaxed),
            },
            utxo_only: self.utxo_only,
        }
    }

    /// Create a new wallet persisting on file system
    #[deprecated(since = "0.16.0", note = "please use `WolletBuilder` instead")]
    pub fn with_fs_persist<P: AsRef<Path>>(
        network: Network,
        descriptor: WolletDescriptor,
        datadir: P,
    ) -> Result<Self, Error> {
        WolletBuilder::new(network, descriptor)
            .with_legacy_fs_store(datadir)?
            .build()
    }

    /// Create a new wallet which does not persist anything
    #[deprecated(since = "0.16.0", note = "please use `WolletBuilder` instead")]
    pub fn without_persist(network: Network, descriptor: WolletDescriptor) -> Result<Self, Error> {
        Self::new(network, Arc::new(FakeStore::new()), descriptor)
    }

    /// Get the network policy asset
    pub fn policy_asset(&self) -> AssetId {
        *self.network.policy_asset()
    }

    /// Creates a transaction builder with a reference to this wallet
    pub fn tx_builder(&self) -> WolletTxBuilder<'_> {
        WolletTxBuilder::new(self)
    }

    /// Get the network
    pub fn network(&self) -> Network {
        self.network
    }

    /// Get a reference of the wallet descriptor
    pub fn descriptor(&self) -> Result<&ConfidentialDescriptor<DescriptorPublicKey>, Error> {
        self.descriptor.ct_descriptor()
    }

    /// Get a copy of the wallet descriptor
    pub fn wollet_descriptor(&self) -> WolletDescriptor {
        self.descriptor.clone()
    }

    /// Get the blockchain tip
    pub fn tip(&self) -> Tip {
        let (height, hash) = self.cache.tip;
        let timestamp = self.cache.timestamps.get(&height).cloned();
        Tip {
            height,
            hash,
            timestamp,
        }
    }

    fn inner_address(&self, chain: Chain, index: Option<u32>) -> Result<AddressResult, Error> {
        let has_descriptor = self.descriptor.spk_count().is_none();
        if has_descriptor && !self.descriptor.has_wildcard() && index.is_some() {
            // Descriptor wollets without wildcard cannot pass the index
            return Err(Error::IndexWithoutWildcard);
        }
        if !has_descriptor && index.is_none() {
            // For wollets without descriptors, caller must pass index
            return Err(Error::UnsupportedWithoutDescriptor);
        }
        let index = match (chain, index) {
            (_, Some(i)) => i,
            (Chain::Internal, None) => self.last_unused_internal(),
            (Chain::External, None) => self.last_unused_external(),
        };

        let address = self
            .descriptor
            .inner_address(index, self.network.address_params(), chain)?;
        Ok(AddressResult::new(address, index))
    }

    /// Get a wallet address
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn address(&self, index: Option<u32>) -> Result<AddressResult, Error> {
        self.inner_address(Chain::External, index)
    }

    /// Get a wallet pegin address
    ///
    /// A pegin address is a Bitcoin address whose funds can be claimed as
    /// Liquid Bitcoin.
    ///
    /// If Some return the address at the given index,
    /// otherwise the last unused address.
    pub fn pegin_address(
        &self,
        index: Option<u32>,
        fed_peg: &FedPeg,
    ) -> Result<PeginAddress, Error> {
        if self.network() != fed_peg.network() {
            return Err(Error::PeginNetworkMismatch {
                wallet: self.network().as_str(),
                fed_peg: fed_peg.network().as_str(),
            });
        }

        let index = self.unwrap_or_last_unused(index);
        self.descriptor.pegin_address(index, fed_peg)
    }

    pub(crate) fn last_unused_external(&self) -> u32 {
        let cache = &self.cache;
        cache.last_unused_external.load(atomic::Ordering::Relaxed)
    }

    pub(crate) fn last_unused_internal(&self) -> u32 {
        let cache = &self.cache;
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
        self.inner_address(Chain::Internal, index)
    }

    fn utxos_inner(&self) -> Result<Vec<WalletTxOut>, Error> {
        let mut utxos = vec![];
        for (outpoint, script_pubkey) in self.cache.unspent() {
            // Technically all these unwrap and error mapping should not be necessary
            // as all these values should be in the cache, but better be safer here and don't panic
            let unblinded = *self
                .cache
                .get_unblinded(outpoint)
                .ok_or_else(|| Error::Generic("missing unblinded".into()))?;
            if is_explicit(&unblinded) {
                continue;
            }
            let height = self.cache.tx_height(&outpoint.txid).unwrap_or(&None);

            // A silent payment script is not derived from the descriptor, so it cannot go
            // through `index()`; it is described by its cache entry instead.
            #[cfg(feature = "silentpayments")]
            if let Some(entry) = self.cache.silent_payment(script_pubkey) {
                utxos.push(self.silent_payment_wallet_txout(entry, unblinded, *height)?);
                continue;
            }

            let index = self
                .index(script_pubkey)
                .ok()
                .ok_or_else(|| Error::Generic("missing index".into()))?;
            let blinding_pubkey = (!is_explicit(&unblinded))
                .then(|| {
                    self.cache
                        .scripts
                        .get(&(index.0, index.1.into()))
                        .and_then(|(_, blinding_pubkey)| *blinding_pubkey)
                })
                .flatten();
            let address = Address::from_script(
                script_pubkey,
                blinding_pubkey,
                self.network().address_params(),
            )
            .ok_or_else(|| Error::Generic("invalid scriptpubkey".into()))?;
            utxos.push(WalletTxOut {
                outpoint: *outpoint,
                script_pubkey: script_pubkey.clone(),
                height: *height,
                unblinded,
                wildcard_index: index.1,
                ext_int: index.0,
                is_spent: false,
                address,
            })
        }
        Ok(utxos)
    }

    fn txos_inner(&self) -> Result<Vec<WalletTxOut>, Error> {
        let mut txos = vec![];
        let unspent = self.cache.unspent();
        for (tx_id, height) in self.cache.sorted_txids() {
            let tx = self
                .cache
                .tx(tx_id)
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
                    (out_point, output, !unspent.contains_key(&out_point))
                })
                .filter_map(|(outpoint, output, is_spent)| {
                    let unblinded = *self.cache.get_unblinded(&outpoint)?;

                    // Silent payment outputs are described by their cache entry, not by a
                    // derivation path. They must appear here too, not just in `utxos()`:
                    // `txos_inner` is what transaction history and per-transaction
                    // balances are computed from, so skipping them would show a receiving
                    // transaction with no incoming amount.
                    #[cfg(feature = "silentpayments")]
                    if let Some(entry) = self.cache.silent_payment(&output.script_pubkey) {
                        let mut txo = self
                            .silent_payment_wallet_txout(entry, unblinded, *height)
                            .ok()?;
                        txo.is_spent = is_spent;
                        return Some(txo);
                    }

                    let index = self.index(&output.script_pubkey).ok()?;
                    let blinding_pubkey = (!is_explicit(&unblinded))
                        .then(|| {
                            self.cache
                                .scripts
                                .get(&(index.0, index.1.into()))
                                .and_then(|(_, blinding_pubkey)| *blinding_pubkey)
                        })
                        .flatten();
                    let address = Address::from_script(
                        &output.script_pubkey,
                        blinding_pubkey,
                        self.network().address_params(),
                    )?;
                    Some(WalletTxOut {
                        outpoint,
                        script_pubkey: output.script_pubkey.clone(),
                        height: *height,
                        unblinded,
                        wildcard_index: index.1,
                        ext_int: index.0,
                        is_spent,
                        address,
                    })
                });
            txos.extend(tx_txos);
        }

        Ok(txos)
    }

    /// Render a found silent payment output as an ordinary [`WalletTxOut`].
    ///
    /// Two fields cannot mean what they usually do, because this output has no
    /// derivation path:
    ///
    /// - `wildcard_index` carries the silent payment output counter `k`. It is not a
    ///   BIP-32 child index, and deriving the wallet descriptor at `k` yields an
    ///   unrelated script.
    /// - `ext_int` is a classification from the BIP-352 label rather than a chain in the
    ///   descriptor: change-labeled outputs report [`Chain::Internal`].
    ///
    /// The address is confidential, built from the output's own blinding pubkey — the
    /// blinding key comes from the shared secret, so the wallet's SLIP-77 key has
    /// nothing to do with it.
    #[cfg(feature = "silentpayments")]
    fn silent_payment_wallet_txout(
        &self,
        entry: &crate::silentpayments::SilentPaymentCacheEntry,
        unblinded: TxOutSecrets,
        height: Option<Height>,
    ) -> Result<WalletTxOut, Error> {
        let address = Address::from_script(
            &entry.script_pubkey,
            Some(entry.blinding_pubkey),
            self.network().address_params(),
        )
        .ok_or_else(|| Error::Generic("invalid silent payment scriptpubkey".into()))?;
        Ok(WalletTxOut {
            outpoint: entry.outpoint,
            script_pubkey: entry.script_pubkey.clone(),
            height,
            unblinded,
            wildcard_index: entry.k,
            ext_int: entry.chain(),
            is_spent: false,
            address,
        })
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
        let mut utxos = vec![];
        for outpoint in self.cache.unspent().keys() {
            // Technically all these unwrap and error mapping should not be necessary
            // as all these values should be in the cache, but better be safer here and don't panic
            let unblinded = *self
                .cache
                .get_unblinded(outpoint)
                .ok_or_else(|| Error::Generic("missing unblinded".into()))?;
            if !is_explicit(&unblinded) {
                continue;
            }
            let tx = self
                .cache
                .tx(&outpoint.txid)
                .ok_or_else(|| Error::Generic("missing tx".into()))?;
            let txout = tx
                .output
                .get(outpoint.vout as usize)
                .ok_or_else(|| Error::Generic("missing output".into()))?;
            let tx_ = if self.is_segwit() {
                None
            } else {
                Some(tx.clone())
            };
            utxos.push(ExternalUtxo {
                outpoint: *outpoint,
                txout: txout.clone(),
                tx: tx_,
                unblinded,
                max_weight_to_satisfy: self.max_weight_to_satisfy,
            });
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
            if self.cache.paths.contains_key(&output.script_pubkey) {
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
        for (txid, tx) in self.cache.all_txs() {
            for (vout, o) in tx.output.iter().enumerate() {
                let outpoint = OutPoint::new(txid, vout as u32);
                if !o.script_pubkey.is_empty()
                    && self.cache.paths.contains_key(&o.script_pubkey)
                    && self.cache.get_unblinded(&outpoint).is_none()
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
    ///
    /// Note: this function might return spent transaction outputs too
    pub fn unblind_utxos_with(
        &self,
        blinding_key: bitcoin::secp256k1::SecretKey,
    ) -> Result<Vec<ExternalUtxo>, Error> {
        let mut utxos = vec![];
        for (txid, tx) in self.cache.all_txs() {
            for (i, txout) in tx.output.iter().enumerate() {
                if self.cache.paths.contains_key(&txout.script_pubkey) {
                    let outpoint = OutPoint::new(txid, i as u32);
                    if self.cache.get_unblinded(&outpoint).is_none() {
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
        let mut new = HashMap::new();
        let mut txos = vec![];
        for (txid, tx) in self.cache.all_txs() {
            for (vout, txout) in tx.output.iter().enumerate() {
                if self.cache.paths.contains_key(&txout.script_pubkey) {
                    let outpoint = OutPoint::new(txid, vout as u32);
                    if self.cache.get_unblinded(&outpoint).is_none() {
                        if let Ok(unblinded) = try_unblind(txout, &self.descriptor) {
                            new.insert(outpoint, unblinded);
                            txos.push(outpoint);
                        }
                    }
                }
            }
        }
        self.cache.extend_unblinded(new);
        // Newly unblinded outpoints can also affect the "cannot unblind" status of the
        // transactions that spend them as inputs, so refresh all of them rather than just
        // the transactions that own the outpoints above.
        let all_txids: Vec<Txid> = self.cache.all_txids().iter().cloned().collect();
        self.cache.refresh_cannot_unblind(all_txids)?;
        Ok(txos)
    }

    /// Get all unblinded values
    ///
    /// **Experimental**: this API might change without notice.
    pub fn all_unblinded(&self) -> HashMap<OutPoint, TxOutSecrets> {
        self.cache.all_unblinded().clone()
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

    /// Get the asset identifiers ever owned by the wallet
    pub fn assets_owned(&self) -> Result<HashSet<AssetId>, Error> {
        Ok(self
            .cache
            .all_unblinded()
            .values()
            .map(|u| u.asset)
            .collect())
    }

    /// Get the wallet transactions with pagination
    pub fn transactions_paginated(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<WalletTx>, Error> {
        let mut txs = vec![];

        let txos = self.txos_map()?;
        for (txid, height) in self.cache.sorted_txids().skip(offset).take(limit) {
            let tx = self
                .cache
                .tx(txid)
                .ok_or_else(|| Error::Generic(format!("list_tx no tx {txid}")))?;

            let balance = tx_balance(*txid, &tx, &txos);
            let inputs = tx_inputs(&tx, &txos);
            let outputs = tx_outputs(*txid, &tx, &txos);
            if balance.is_empty()
                && inputs.iter().all(|i| i.is_none())
                && outputs.iter().all(|o| o.is_none())
            {
                // Transaction has no output or input that the wollet can unblind or
                // match as explicit, ignore this transaction
                continue;
            }
            let fee = tx_fee(&tx);
            let policy_asset = self.policy_asset();
            let type_ = tx_type(&tx, &policy_asset, &balance, fee);
            let timestamp = height.and_then(|h| self.cache.timestamps.get(&h).cloned());
            txs.push(WalletTx {
                tx,
                txid: *txid,
                height: *height,
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
    ///
    /// Note: this returns any transaction that the wallet has in its storage,
    /// which in some circumstances might include non-wallet txs.
    pub fn transaction(&self, txid: &Txid) -> Result<Option<WalletTx>, Error> {
        let height = self.cache.tx_height(txid).unwrap_or(&None);
        if let Some(tx) = self.cache.tx(txid) {
            let txos = self.txos_map()?;

            let balance = tx_balance(*txid, &tx, &txos);
            let fee = tx_fee(&tx);
            let policy_asset = self.policy_asset();
            let type_ = tx_type(&tx, &policy_asset, &balance, fee);
            let timestamp = height.and_then(|h| self.cache.timestamps.get(&h).cloned());
            let inputs = tx_inputs(&tx, &txos);
            let outputs = tx_outputs(*txid, &tx, &txos);

            Ok(Some(WalletTx {
                tx,
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
            balance: pset_balance(pset, self.descriptor()?, self.network.address_params())?,
            sig_details: pset_signatures(pset),
            issuances: pset_issuances(pset),
        })
    }

    pub(crate) fn index(&self, script_pubkey: &Script) -> Result<(Chain, u32), Error> {
        let (ext_int, index) = self
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
        if self.descriptor.spk_count().is_some() {
            // The wollet does not have a descriptor, nothing to add
            // Since this method is also called internally, we prefer not to raise an error
            return Ok(());
        }
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
        self.descriptor()?.descriptor.for_each_key(|k| {
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
        let _ = self.descriptor().map(|d| {
            d.descriptor.for_each_key(|k| {
                // xpub without key origin and single pubkey unexpectedly return a master fingerprint,
                // see tests below for the actual behaviour.
                // This should not be dangerous though, worst case is that we report a signer that
                // cannot sign.
                signers.push(k.master_fingerprint());
                true
            })
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

        // ELIP-101 key written by TxBuilder: https://github.com/ElementsProject/ELIPs/blob/main/elip-0101.mediawiki
        let genesis_hash = get_genesis_hash(pset);

        // genesis_hash is only used for BIP341 (taproot) sighash computation
        let result = pset.finalize_mut(&EC, genesis_hash);

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

    /// A deterministic value derived from the descriptor, the config and the content of this wollet,
    /// including what's in the wallet cache (transactions etc)
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
        self.cache.tip == (0, BlockHash::all_zeros())
    }
}

/// Silent-payment wallet operations using scan-only material.
#[cfg(feature = "silentpayments")]
impl Wollet {
    /// Returns the wallet's scan-only silent-payment material, if configured.
    pub fn silent_payment_material(
        &self,
    ) -> Option<&crate::silentpayments::SilentPaymentScanMaterial> {
        self.silent_payment_material.as_ref()
    }

    /// Returns the height through which silent-payment discovery has scanned.
    pub fn silent_payments_scanned_to(&self) -> Option<Height> {
        self.cache.silent_payments_scanned_to
    }

    /// Returns the wallet's reusable silent-payment address.
    pub fn silent_payment_address(&self) -> Result<String, Error> {
        use crate::silentpayments::SilentPaymentScan as _;
        Ok(self
            .require_silent_payment_material()?
            .address()
            .encode(self.network))
    }

    /// A labeled silent-payment address (BIP-352 labels), e.g.
    /// [`crate::silentpayments::CHANGE_LABEL`] for change.
    pub fn labeled_silent_payment_address(&self, label: u32) -> Result<String, Error> {
        use crate::silentpayments::SilentPaymentScan as _;
        Ok(self
            .require_silent_payment_material()?
            .labeled_address(label)
            .encode(self.network))
    }

    /// The scan material, or an error explaining that the wallet has none.
    pub(crate) fn require_silent_payment_material(
        &self,
    ) -> Result<&crate::silentpayments::SilentPaymentScanMaterial, Error> {
        self.silent_payment_material
            .as_ref()
            .ok_or(Error::MissingSilentPaymentKeys)
    }

    /// Find the silent payments to this wallet among `txs`, returning them unblinded and
    /// ready to spend.
    ///
    /// This is the low-level entry point, for callers holding transactions from a source
    /// of their own. A normal sync reaches it via
    /// [`crate::clients::blocking::BlockchainBackend::full_scan()`], after which the
    /// results are in [`Wollet::utxos()`] and [`Wollet::balance()`] and need no special
    /// handling to spend; use this directly only to scan transactions the wallet's
    /// backend cannot supply.
    ///
    /// Transactions must be passed in because a silent-payment scriptPubKey is derived
    /// from a shared secret and so matches none of the wallet's descriptor-derived
    /// scripts: no script-history query can surface it, and the transactions have to come
    /// from a source that does not filter by descriptor (a full node, a tweak/index
    /// server, or a block feed).
    ///
    /// `prevout_script` resolves the scriptPubKey each input spends, which is what the
    /// input pubkeys are recovered from; returning `None` makes that input contribute
    /// no key. When a tweak server already publishes `T = input_hash·A`, prefer
    /// [`crate::silentpayments::SilentPaymentTxScanner::scan_tx_with_tweak()`] and skip
    /// the prevout lookups entirely.
    ///
    /// `labels` are the BIP-352 labels to watch in addition to the plain address.
    ///
    /// Results are returned, **not** applied to the wallet: this method takes `&self`.
    /// Feed them to [`Wollet::apply_silent_payments()`] to have them show up in the
    /// balance, or spend them standalone via
    /// [`crate::TxBuilder::add_silent_payment_utxos()`] — which carries the spend tweak
    /// through to the PSET, as `external_utxo()` alone cannot.
    pub fn scan_silent_payments<'a, F>(
        &self,
        txs: &[Transaction],
        labels: impl IntoIterator<Item = u32>,
        mut prevout_script: F,
    ) -> Result<Vec<crate::silentpayments::SilentPaymentUtxo>, Error>
    where
        F: FnMut(&OutPoint) -> Option<&'a Script>,
    {
        let material = *self.require_silent_payment_material()?;
        let scanner =
            crate::silentpayments::SilentPaymentTxScanner::new(material).with_labels(labels);
        Ok(txs
            .iter()
            .flat_map(|tx| scanner.scan_tx(tx, &mut prevout_script))
            .collect())
    }

    /// Record found silent payments as wallet outputs.
    ///
    /// After this they are ordinary wallet outputs: they appear in [`Wollet::utxos()`],
    /// [`Wollet::balance()`] and [`Wollet::txos()`], coin selection will spend them, and
    /// the fee estimate uses their Taproot satisfaction weight rather than the
    /// descriptor's.
    ///
    /// The transaction containing each output must already be in the wallet (a scan
    /// applies the transactions it fetched before applying what it found), otherwise the
    /// output has no confirmation height and no spend/unspend bookkeeping to hang off;
    /// such entries are reported as [`Error::MissingTransaction`] rather than stored in a
    /// state where the balance would count an output the wallet cannot locate.
    ///
    /// Only the spend **tweak** is stored, never the spend key — see
    /// [`crate::silentpayments::SilentPaymentCacheEntry`]. Signing re-derives the key
    /// from the wallet's scan keys.
    pub fn apply_silent_payments(
        &mut self,
        found: &[crate::silentpayments::SilentPaymentUtxo],
    ) -> Result<(), Error> {
        // Check every output first: applying half of them would leave the balance
        // counting outputs whose transaction the wallet cannot locate.
        for utxo in found {
            if self.cache.tx(&utxo.outpoint.txid).is_none() {
                return Err(Error::MissingTransaction);
            }
        }

        for utxo in found {
            self.cache
                .insert_silent_payment(utxo.cache_entry(), utxo.unblinded);
        }
        Ok(())
    }
}

fn is_explicit(txoutsecrets: &TxOutSecrets) -> bool {
    txoutsecrets.asset_bf == AssetBlindingFactor::zero()
        && txoutsecrets.value_bf == ValueBlindingFactor::zero()
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
            if !is_explicit(&txout.unblinded) {
                *balance.entry(txout.unblinded.asset).or_default() += txout.unblinded.value as i64;
            }
        }
    }
    for input in &tx.input {
        if let Some(txout) = txos.get(&input.previous_output) {
            if !is_explicit(&txout.unblinded) {
                *balance.entry(txout.unblinded.asset).or_default() -= txout.unblinded.value as i64;
            }
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
            WolletBuilder::new(Network::default_regtest(), desc).build()?,
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
    use crate::elements::bitcoin::network::Network as BitcoinNetwork;
    use crate::elements::AddressParams;
    use crate::{DownloadTxResult, Update};
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
    fn test_directory_id_hash_empty_input_regression() {
        let got = <crate::wollet::DirectoryIdHash as crate::hashes::Hash>::hash(b"").to_string();
        let exp = "318c87cf9bafe6a65c82e0124e3c2ab31e656d4f64ee8459017b1f9dd91ba818";
        assert_eq!(got, exp);
    }

    #[test]
    fn test_blinding_private() {
        // Get a confidential address from a "view" descriptor
        let seed = [0u8; 16];
        let xprv = Xpriv::new_master(BitcoinNetwork::Regtest, &seed).unwrap();
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
        WolletBuilder::new(Network::TestnetLiquid, desc)
            .build()
            .unwrap()
    }

    /// Scan material for testnet `account`, derived from a fixed master key.
    ///
    /// This duplicates what `lwk_signer::SwSigner` does, deliberately: `lwk_wollet`
    /// must not depend on `lwk_signer` (the whole point of the boundary), so a test
    /// that needs realistic material has to derive it here. Note what it produces is
    /// still only `b_scan` + `B_spend` — `b_spend` is derived, reduced to a point,
    /// and dropped, exactly as the signer does.
    #[cfg(feature = "silentpayments")]
    fn test_scan_material(account: u32) -> crate::silentpayments::SilentPaymentScanMaterial {
        use crate::silentpayments::{SilentPaymentAccount, SilentPaymentScanMaterial};

        let master = Xpriv::new_master(BitcoinNetwork::Testnet, &[0x42; 32]).unwrap();
        let acct = SilentPaymentAccount::liquid_testnet(account);
        let derive = |path| master.derive_priv(&EC, &path).unwrap().private_key;

        SilentPaymentScanMaterial::new(
            acct,
            derive(acct.scan_path()),
            derive(acct.spend_path()).public_key(&EC),
        )
    }

    fn fast_tempdir() -> tempfile::TempDir {
        tempfile::tempdir_in("/dev/shm").unwrap_or_else(|_| tempfile::tempdir().unwrap())
    }

    #[cfg(feature = "silentpayments")]
    #[test]
    fn silent_payment_material_changes_wallet_identity() {
        use crate::silentpayments::CHANGE_LABEL;

        let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let desc = format!("ct({view_key},elwpkh({xpub}/*))");

        // The receiver's scan material, obtained the way a real wallet obtains it:
        // from a signer, which derives both BIP-352 branches and exports only the
        // scan-only half.
        let network = Network::TestnetLiquid;
        let keys = test_scan_material(0);

        // A wallet without scan material refuses to hand out an address it could never
        // scan for, rather than returning one that would silently swallow funds.
        let keyless = new_wollet(&desc);
        assert!(matches!(
            keyless.silent_payment_address(),
            Err(Error::MissingSilentPaymentKeys)
        ));

        let wollet = WolletBuilder::new(network, desc.parse().unwrap())
            .with_silent_payment_material(keys)
            .build()
            .unwrap();

        // Scan material participates in the wallet identity, so a cache built without it
        // is not mistaken for one built with it.
        assert_ne!(
            wollet.status(),
            keyless.status(),
            "scan material must change `status()`, else toggling it reuses a stale cache"
        );

        // ...and so does *which* material it is. Two accounts of the same signer
        // discover different outputs, so their caches are not interchangeable either.
        let other_account = WolletBuilder::new(network, desc.parse().unwrap())
            .with_silent_payment_material(test_scan_material(1))
            .build()
            .unwrap();
        assert_ne!(
            wollet.status(),
            other_account.status(),
            "a different silent payment account must change `status()`"
        );

        // The account coordinates are hashed in their own right, not merely implied
        // by the keys. Today distinct accounts always derive distinct points, so the
        // assertion above would hold even if the coordinates were ignored; this one
        // isolates them by holding the keys fixed and changing only the account.
        //
        // That combination cannot arise from a signer, which is the point: it is what
        // a future derivation change could produce, and identity should already be
        // correct when it does rather than needing to be fixed afterwards.
        let same_keys_other_account = WolletBuilder::new(network, desc.parse().unwrap())
            .with_silent_payment_material(crate::silentpayments::SilentPaymentScanMaterial::new(
                lwk_common::silentpayments::SilentPaymentAccount::liquid_testnet(9),
                keys.scan_seckey(),
                keys.spend_pubkey(),
            ))
            .build()
            .unwrap();
        assert_ne!(
            wollet.status(),
            same_keys_other_account.status(),
            "account coordinates must contribute to `status()` on their own"
        );

        // The published address is reusable and network-tagged.
        let address = wollet.silent_payment_address().unwrap();
        assert!(address.starts_with("tlqsp1"), "unexpected hrp: {address}");
        assert_eq!(
            address,
            wollet.silent_payment_address().unwrap(),
            "an SP address is reusable, not index-consuming"
        );
        // Labeled addresses differ from the plain one but share the scan key.
        let change_address = wollet.labeled_silent_payment_address(CHANGE_LABEL).unwrap();
        assert_ne!(change_address, address);
    }

    #[cfg(feature = "silentpayments")]
    #[test]
    fn applying_a_silent_payment_requires_its_transaction() {
        use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
        use crate::silentpayments::test_fixture::SpPaymentBuilder;

        let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let desc = format!("ct({view_key},elwpkh({xpub}/*))");
        let network = Network::TestnetLiquid;
        let keys = test_scan_material(0);

        let mut wollet = WolletBuilder::new(network, desc.parse().unwrap())
            .with_silent_payment_material(keys)
            .build()
            .unwrap();

        // A stranger pays the wallet's SP address.
        let asset = *network.policy_asset();
        let value = 123_456u64;
        let payment = SpPaymentBuilder::new()
            .with_inputs(&[(Data::outpoint(0x31, 0), Data::secret_key(0xB1))])
            .with_value(value)
            .with_asset(asset)
            .without_extra_output()
            .build_for(&keys);
        let tx = payment.tx.clone();

        let found = wollet
            .scan_silent_payments(std::slice::from_ref(&tx), [], payment.prevout_lookup())
            .unwrap();
        assert_eq!(found.len(), 1);

        // Applying before the transaction is known must fail rather than record an
        // output whose transaction the wallet cannot locate.
        assert!(matches!(
            wollet.apply_silent_payments(&found),
            Err(Error::MissingTransaction)
        ));
        assert!(wollet.balance().unwrap().values().all(|v| *v == 0));
    }

    /// A silent payment spent *standalone* — never applied to the wallet — must reach a
    /// signer just as well annotated as one the wallet is tracking.
    ///
    /// This is the path `scan_silent_payments()` documents for callers holding their own
    /// transactions. Funding it was always possible through `external_utxo()`, but an
    /// `ExternalUtxo` has nowhere to carry the spend tweak, so the resulting input had
    /// no `bip32_derivation` (it cannot) *and* no metadata (it should have) — an input
    /// no signer could complete, from an API that looked like it worked.
    #[cfg(feature = "silentpayments")]
    #[test]
    fn a_standalone_silent_payment_reaches_the_signer_annotated() {
        use crate::silentpayments::test_fixture::SilentPaymentTestData as Data;
        use crate::silentpayments::test_fixture::SpPaymentBuilder;
        use lwk_common::silentpayments::SilentPaymentInputMeta;

        let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let desc = format!("ct({view_key},elwpkh({xpub}/*))");
        let network = Network::TestnetLiquid;
        let keys = test_scan_material(0);
        let asset = *network.policy_asset();

        let mut wollet = WolletBuilder::new(network, desc.parse().unwrap())
            .with_silent_payment_material(keys)
            .build()
            .unwrap();

        // A stranger pays the wallet's SP address, and separately the wallet holds
        // ordinary money so the transaction has a descriptor-derived input to contrast
        // against.
        let sp_value = 200_000u64;
        let payment = SpPaymentBuilder::new()
            .with_inputs(&[(Data::outpoint(0x41, 0), Data::secret_key(0xB4))])
            .with_value(sp_value)
            .with_asset(asset)
            .without_extra_output()
            .build_for(&keys);
        let tx = payment.tx.clone();

        let found = wollet
            .scan_silent_payments(std::slice::from_ref(&tx), [], payment.prevout_lookup())
            .unwrap();
        assert_eq!(found.len(), 1);

        // The transaction is known to the wallet (it has to be, to spend from it) but
        // the payment is deliberately NOT applied: this is the standalone path, so the
        // cache must not be what makes the annotation work.
        wollet
            .cache
            .update(
                &[(tx.txid(), Some(100u32))],
                &[],
                &[(tx.txid(), tx.clone())],
                false,
                vec![],
                false,
            )
            .unwrap();
        assert!(
            wollet.utxos().unwrap().is_empty(),
            "the payment must not be wallet money in this test"
        );

        // Spend it standalone. Every satoshi has to come from the silent payment,
        // since the wallet has nothing else.
        let recipient_addr = wollet.address(Some(0)).unwrap().address().clone();
        let pset = wollet
            .tx_builder()
            .add_silent_payment_utxos(found.clone())
            .unwrap()
            .add_recipient(&recipient_addr, 1_000, asset)
            .unwrap()
            .finish()
            .unwrap();

        let sp_index = pset
            .inputs()
            .iter()
            .position(|i| {
                i.previous_txid == found[0].outpoint.txid
                    && i.previous_output_index == found[0].outpoint.vout
            })
            .expect("the standalone silent payment must fund the transaction");

        let sp_input = &pset.inputs()[sp_index];
        assert!(
            sp_input.bip32_derivation.is_empty(),
            "an SP input has no derivation path; the metadata is what stands in for it"
        );

        let meta = SilentPaymentInputMeta::read(sp_input)
            .expect("a standalone silent payment input must carry signing metadata");
        assert_eq!(meta.account(), keys.account());
        assert_eq!(meta.expected_spend_pubkey(), keys.spend_pubkey());
        assert_eq!(&meta.spend_tweak(), found[0].spend_tweak.as_scalar());

        // The metadata must survive serialization: a PSET exists to be handed to
        // another process, and metadata that only round-trips in memory is useless.
        let reparsed: PartiallySignedTransaction = pset.to_string().parse().unwrap();
        assert_eq!(
            SilentPaymentInputMeta::read(&reparsed.inputs()[sp_index]).unwrap(),
            meta
        );
    }

    /// A silent payment UTXO handed to a wallet that holds no scan material must be
    /// refused, not quietly funded into an unsignable PSET.
    ///
    /// Such a wallet could not have found the output, so it cannot vouch for the tweak
    /// or name the account a signer should use. Skipping the annotation would produce
    /// exactly the failure this whole change exists to remove.
    #[cfg(feature = "silentpayments")]
    #[test]
    fn spending_a_silent_payment_without_scan_material_is_refused() {
        use crate::silentpayments::{SilentPaymentOutput, SilentPaymentUtxo, SpendTweak};

        let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let desc = format!("ct({view_key},elwpkh({xpub}/*))");
        let network = Network::TestnetLiquid;

        // Built WITHOUT `with_silent_payment_material`.
        let wollet = WolletBuilder::new(network, desc.parse().unwrap())
            .build()
            .unwrap();

        let stranger = crate::secp256k1::SecretKey::from_slice(&[0xE1; 32]).unwrap();
        let pk = stranger.public_key(&EC);
        let utxo = SilentPaymentUtxo {
            outpoint: OutPoint::new(
                <Txid as crate::hashes::Hash>::from_byte_array([0xF2; 32]),
                0,
            ),
            txout: crate::elements::TxOut {
                asset: crate::elements::confidential::Asset::Explicit(*network.policy_asset()),
                value: crate::elements::confidential::Value::Explicit(50_000),
                nonce: Default::default(),
                script_pubkey: Script::new_v1_p2tr_tweaked(
                    crate::elements::schnorr::TweakedPublicKey::new(pk.x_only_public_key().0),
                ),
                witness: Default::default(),
            },
            unblinded: TxOutSecrets::new(
                *network.policy_asset(),
                AssetBlindingFactor::zero(),
                50_000,
                ValueBlindingFactor::zero(),
            ),
            k: 0,
            label: None,
            output: SilentPaymentOutput {
                spend_pubkey: pk,
                blinding_pubkey: pk,
                blinding_seckey: stranger,
            },
            spend_tweak: SpendTweak::from_be_bytes([0x3C; 32]).unwrap(),
        };

        let recipient_addr = wollet.address(Some(0)).unwrap().address().clone();
        let err = wollet
            .tx_builder()
            .add_silent_payment_utxos(vec![utxo])
            .unwrap()
            .add_recipient(&recipient_addr, 1_000, *network.policy_asset())
            .unwrap()
            .finish()
            .expect_err("a wallet with no scan material cannot vouch for a silent payment");
        assert!(
            matches!(err, Error::MissingSilentPaymentKeys),
            "unexpected error: {err}"
        );
    }

    /// A cache entry whose tweak does not describe its own script must be refused at
    /// build time, not annotated and shipped to a signer.
    ///
    /// The signer would catch it — that check is the real gate. But a PSET that is
    /// guaranteed to fail signing is not a useful thing to hand back to a caller, and
    /// the failure would surface far from its cause, looking like a signer bug rather
    /// than a corrupt or foreign cache entry.
    #[cfg(feature = "silentpayments")]
    #[test]
    fn a_corrupt_cache_entry_is_refused_before_it_reaches_a_signer() {
        use crate::silentpayments::{SilentPaymentCacheEntry, SpendTweak};

        let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let desc = format!("ct({view_key},elwpkh({xpub}/*))");
        let keys = test_scan_material(0);

        let mut wollet = WolletBuilder::new(Network::TestnetLiquid, desc.parse().unwrap())
            .with_silent_payment_material(keys)
            .build()
            .unwrap();

        // A well-formed Taproot script paired with a tweak that has nothing to do with
        // it — the shape a corrupted or cross-wallet cache entry would have.
        let stranger = crate::secp256k1::SecretKey::from_slice(&[0xC7; 32])
            .unwrap()
            .public_key(&EC);
        let entry = SilentPaymentCacheEntry {
            outpoint: OutPoint::new(
                <Txid as crate::hashes::Hash>::from_byte_array([0xD1; 32]),
                0,
            ),
            script_pubkey: Script::new_v1_p2tr_tweaked(
                crate::elements::schnorr::TweakedPublicKey::new(stranger.x_only_public_key().0),
            ),
            k: 0,
            label: None,
            spend_tweak: SpendTweak::from_be_bytes([0x5A; 32]).unwrap(),
            blinding_pubkey: stranger,
        };

        assert!(
            !entry.verify(&keys.spend_pubkey()),
            "fixture must be a genuinely inconsistent entry"
        );

        // It has to be *in the cache*: the annotation path keys off the cache to decide
        // whether an input is a silent payment at all.
        let script = entry.script_pubkey.clone();
        let outpoint = entry.outpoint;
        wollet.cache.insert_silent_payment(
            entry,
            TxOutSecrets::new(
                *Network::TestnetLiquid.policy_asset(),
                AssetBlindingFactor::zero(),
                1_000,
                ValueBlindingFactor::zero(),
            ),
        );

        // The witness utxo is what verification reads the script from — the annotator
        // checks the tweak against the output the PSET actually spends, not against
        // whatever the cache claims the script is.
        let mut pset = PartiallySignedTransaction::new_v2();
        let mut input = crate::elements::pset::Input::from_prevout(outpoint);
        input.witness_utxo = Some(crate::elements::TxOut {
            asset: crate::elements::confidential::Asset::Explicit(
                *Network::TestnetLiquid.policy_asset(),
            ),
            value: crate::elements::confidential::Value::Explicit(1_000),
            nonce: Default::default(),
            script_pubkey: script,
            witness: Default::default(),
        });
        pset.add_input(input);

        let err = crate::silentpayments::SilentPaymentPsetAnnotator::for_builder(&wollet, &[])
            .expect("the wallet has scan material")
            .expect("a wallet with material always gets an annotator")
            .annotate(&mut pset)
            .expect_err("an inconsistent cache entry must not be annotated");
        assert!(
            format!("{err}").contains("does not verify"),
            "unexpected error: {err}"
        );

        // And nothing was written: a refused input must not carry half-formed metadata.
        assert!(
            lwk_common::silentpayments::SilentPaymentInputMeta::read(&pset.inputs()[0]).is_err()
        );
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

        let tempdir = fast_tempdir();
        let mut wollet = WolletBuilder::new(Network::TestnetLiquid, desc.clone())
            .with_legacy_fs_store(&tempdir)
            .unwrap()
            .build()
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
            unspent: Vec::new(),
            last_unused: Default::default(),
            #[cfg(feature = "silentpayments")]
            silent_payments: None,
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
        let restored_wollet = WolletBuilder::new(Network::TestnetLiquid, desc)
            .with_legacy_fs_store(&tempdir)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(wollet.status(), restored_wollet.status());
    }

    #[test]
    fn test_with_fs_persist_loads_encrypted_update_backward_compat() {
        let desc: WolletDescriptor = lwk_test_util::wollet_descriptor_string2().parse().unwrap();
        let enc_bytes = lwk_test_util::update_test_vector_encrypted_bytes2();
        let expected_update = Update::deserialize_decrypted(&enc_bytes, &desc).unwrap();

        let tempdir = fast_tempdir();
        let mut update_path = tempdir.path().to_path_buf();
        update_path.push(Network::TestnetLiquid.as_str());
        update_path.push("enc_cache");
        update_path.push(
            <crate::wollet::DirectoryIdHash as crate::hashes::Hash>::hash(
                desc.to_string().as_bytes(),
            )
            .to_string(),
        );
        std::fs::create_dir_all(&update_path).unwrap();
        update_path.push("000000000000");
        std::fs::write(update_path, &enc_bytes).unwrap();

        let wollet = WolletBuilder::new(Network::TestnetLiquid, desc)
            .with_legacy_fs_store(tempdir.path())
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(wollet.updates().unwrap().len(), 1);
        assert_eq!(wollet.tip().height(), 1360180);
        assert_eq!(wollet.updates().unwrap()[0], expected_update);
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

    /// Test that verifies the merge test updates can be deserialized and applied correctly.
    /// This is a unit test that uses hard-coded binary data (no external nodes required).
    #[test]
    fn test_merge_updates_deserialize() {
        use lwk_test_util::{
            update_merge_test_1, update_merge_test_2, update_merge_test_3,
            update_merge_test_descriptor,
        };

        // Load the descriptor
        let desc_str = update_merge_test_descriptor();
        let desc: WolletDescriptor = desc_str.parse().unwrap();

        // Load and deserialize the three updates
        let update1_bytes = update_merge_test_1();
        let update2_bytes = update_merge_test_2();
        let update3_bytes = update_merge_test_3();

        let update1 = Update::deserialize(&update1_bytes).unwrap();
        let update2 = Update::deserialize(&update2_bytes).unwrap();
        let update3 = Update::deserialize(&update3_bytes).unwrap();

        // Verify updates can be applied sequentially
        let mut wollet = WolletBuilder::new(Network::default_regtest(), desc.clone())
            .build()
            .unwrap();

        // After update 1: should be empty (tip only update)
        wollet.apply_update(update1).unwrap();
        let balance1 = wollet.balance().unwrap();
        let txs1 = wollet.transactions().unwrap();
        let utxos1 = wollet.utxos().unwrap();
        assert!(
            balance1.is_empty() || balance1.values().all(|&v| v == 0),
            "Balance should be 0 after update 1"
        );
        assert_eq!(txs1.len(), 0, "Should have 0 transactions after update 1");
        assert_eq!(utxos1.len(), 0, "Should have 0 UTXOs after update 1");

        // After update 2: should have 1,000,000 sats from funding
        wollet.apply_update(update2).unwrap();
        let balance2 = wollet.balance().unwrap();
        let txs2 = wollet.transactions().unwrap();
        let utxos2 = wollet.utxos().unwrap();
        let policy_asset = wollet.policy_asset();
        let btc_balance2 = balance2.get(&policy_asset).copied().unwrap_or(0);
        assert_eq!(
            btc_balance2, 1_000_000,
            "Balance should be 1,000,000 after funding"
        );
        assert_eq!(txs2.len(), 1, "Should have 1 transaction after funding");
        assert_eq!(utxos2.len(), 1, "Should have 1 UTXO after funding");

        // After update 3: should have reduced balance after spending
        wollet.apply_update(update3).unwrap();
        let balance3 = wollet.balance().unwrap();
        let txs3 = wollet.transactions().unwrap();
        let utxos3 = wollet.utxos().unwrap();
        let btc_balance3 = balance3.get(&policy_asset).copied().unwrap_or(0);
        assert!(
            btc_balance3 < 900_000,
            "Balance should be less than 900,000 after spending (sent 100k + fee)"
        );
        assert!(
            btc_balance3 > 800_000,
            "Balance should be more than 800,000 (fee should be less than 100k)"
        );
        assert_eq!(txs3.len(), 2, "Should have 2 transactions after sending");
        assert_eq!(utxos3.len(), 1, "Should have 1 UTXO after sending (change)");
    }

    #[test]
    fn test_desc_no_wildcard_with_index() {
        let k = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let x = "tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M";
        let desc = format!("ct(slip77({k}),elwpkh({x}))");
        let w = new_wollet(&desc);
        let a = w.address(None).unwrap();
        assert_eq!(a.address().to_string(), "tlq1qqtkq6nptvwfycgsvkclsg8uyslwy9pn5mmw6049nmqq02y7l9330a6vmsc5zdfq2xtpyc7tct5rtr80rlvrk7jll6mc5gjfup");
        let err = "Cannot use derivation index when the descriptor has no wildcard";
        assert_eq!(w.address(Some(0)).unwrap_err().to_string(), err);
        // With change
        let desc = format!("ct(slip77({k}),elwpkh({x}/<0;1>))");
        let w = new_wollet(&desc);
        let a = w.address(None).unwrap().address().to_string();
        let c = w.change(None).unwrap().address().to_string();
        assert_ne!(a, c);
        assert_eq!(w.address(Some(0)).unwrap_err().to_string(), err);
        assert_eq!(w.change(Some(0)).unwrap_err().to_string(), err);
    }

    #[test]
    fn fixed_addresses_test() {
        let expected = [
            "lq1qqvxk052kf3qtkxmrakx50a9gc3smqad2ync54hzntjt980kfej9kkfe0247rp5h4yzmdftsahhw64uy8pzfe7cpg4fgykm7cv", //  network: Liquid variant: Wpkh blinding_variant: Slip77
            "lq1qqtmf5e3g4ats3yexwdfn6kfhp9sl68kdl47g75k58rvw2w33zuarwfe0247rp5h4yzmdftsahhw64uy8pzfe7k9s63c7cku58", // network: Liquid variant: Wpkh blinding_variant: Elip151
            "VJLCQwwG8s7qUGhpJkQpkf7wLoK785TcK2cPqka8675FeJB7NEHLto5MUJyhJURGJCbFHA6sb6rgTwbh", // network: Liquid variant: ShWpkh blinding_variant: Slip77
            "VJLD3sfRNBrKyQkJp9KpLqSVtD9YWswXctqzdFhsctaDCwLoUcSato1DfspVSGMbk28avytesWFhiv37", // network: Liquid variant: ShWpkh blinding_variant: Elip151
            "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn", // network: TestnetLiquid variant: Wpkh blinding_variant: Slip77
            "tlq1qqv74shw44vxlpdhtmwqc2zfr5365hm8p6rg8cjnu77w57000dmuc05xy50hsn6vhkm5euwt72x878eq6zxx2z4zm4jus26k72", // network: TestnetLiquid variant: Wpkh blinding_variant: Elip151 i:5
            "vjTwLVioiKrDJ7zZZn9iQQrxP6RPpcvpHBhzZrbdZKKVZE29FuXSnkXdKcxK3qD5t1rYsdxcm9KYRMji", // network: TestnetLiquid variant: ShWpkh blinding_variant: Slip77
            "vjU3guCqyPrnKFXsUhpKPhUyduT6Zjr3b2ukPhE9BpiW4LpehTRvw4FHKxkMw7TRAzE7KhtsnkZ4rPth", // network: TestnetLiquid variant: ShWpkh blinding_variant: Elip151 i:7
            "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq", // network: CustomElements { policy_asset: 0000000000000000000000000000000000000000000000000000000000000000 } variant: Wpkh blinding_variant: Slip77
            "el1qqv74shw44vxlpdhtmwqc2zfr5365hm8p6rg8cjnu77w57000dmuc05xy50hsn6vhkm5euwt72x878eq6zxx2zw8kxk9q8g9ne", // network: CustomElements { policy_asset: 0000000000000000000000000000000000000000000000000000000000000000 } variant: Wpkh blinding_variant: Elip151 i:9
            "AzpmUtw4GMrEsfz6GKx5SKT1DV3qLS3xtSGdKG351rMjGxoUwS6Vsbu3zu2opBiPtjWs1GnE48uMFFnb", // network: CustomElements { policy_asset: 0000000000000000000000000000000000000000000000000000000000000000 } variant: ShWpkh blinding_variant: Slip77
            "AzpsqJR6XRrotoXQBFcgRc52UJ5Y5YyCCHUP96faeMkjn5bzNyzz1uci1EprhTxjBhtRTLiV5k6sWP7j", // network: CustomElements { policy_asset: 0000000000000000000000000000000000000000000000000000000000000000 } variant: ShWpkh blinding_variant: Elip151 i:11
            ];
        let mut i = 0usize;
        let mnemonic = lwk_test_util::TEST_MNEMONIC;

        for network in [
            Network::Liquid,
            Network::TestnetLiquid,
            Network::CustomElements(
                lwk_common::ElementsParamsBuilder::new()
                    .with_policy_asset(AssetId::default())
                    .build()
                    .expect("static"),
            ),
        ] {
            let is_mainnet = matches!(network, Network::Liquid);
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

                    let wollet = WolletBuilder::new(network, desc).build().unwrap();
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
        let wollet = new_wollet(lwk_test_util::PEGIN_TEST_DESC);
        let header = lwk_test_util::liquid_block_header_2_963_520();
        let fed_peg = FedPeg::from_block_header(Network::TestnetLiquid, &header).unwrap();
        let addr = wollet.pegin_address(Some(0), &fed_peg).unwrap();
        assert_eq!(addr.derivation_index(), 0);
        assert_eq!(addr.address().to_string(), lwk_test_util::PEGIN_TEST_ADDR);
        assert_eq!(addr.fed_peg(), &fed_peg);

        let mainnet_fed_peg = FedPeg::from_block_header(Network::Liquid, &header).unwrap();
        assert!(matches!(
            wollet.pegin_address(Some(0), &mainnet_fed_peg),
            Err(Error::PeginNetworkMismatch { .. })
        ));
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
        let network = Network::TestnetLiquid;
        let mut wollet = WolletBuilder::new(network, descriptor).build().unwrap();
        wollet.apply_update(update).unwrap();
        wollet
    }
}
