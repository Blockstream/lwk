use crate::descriptor::Chain;
use crate::elements::{BlockHash, OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::hashes::Hash;
use crate::{BlindingPublicKey, Error};
use elements::bitcoin::bip32::ChildNumber;
use elements_miniscript::{ConfidentialDescriptor, DescriptorPublicKey};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};

pub const BATCH_SIZE: u32 = 20;
pub type Height = u32;
pub type Timestamp = u32;

/// `RawCache` is a cache of wallet data, like wallet transactions.
/// It is fully reconstructable from the CT Descriptor and the blockchain.
pub struct RawCache {
    /// contains all my tx and all prevouts
    pub all_txs: HashMap<Txid, Transaction>,

    /// contains all my script up to an empty batch of BATCHSIZE
    pub paths: HashMap<Script, (Chain, ChildNumber)>,

    /// inverse of `paths`, with the blinding public key for each script
    pub scripts: HashMap<(Chain, ChildNumber), (Script, BlindingPublicKey)>,

    /// contains only my wallet txs with the relative heights (None if unconfirmed)
    pub heights: HashMap<Txid, Option<Height>>,

    /// unblinded values
    pub unblinded: HashMap<OutPoint, TxOutSecrets>,

    /// height and hash of tip of the blockchain
    pub tip: (Height, BlockHash),

    /// Contains the time of blocks at the given height. There are only heights containinig wallet txs
    pub timestamps: HashMap<Height, Timestamp>,

    /// last unused index for external addresses for current descriptor
    pub last_unused_external: AtomicU32,

    /// last unused index for internal addresses (changes) for current descriptor
    pub last_unused_internal: AtomicU32,
}

impl Default for RawCache {
    fn default() -> Self {
        Self {
            all_txs: HashMap::default(),
            paths: HashMap::default(),
            scripts: HashMap::default(),
            heights: HashMap::default(),
            unblinded: HashMap::default(),
            tip: (0, BlockHash::all_zeros()),
            last_unused_internal: 0.into(),
            last_unused_external: 0.into(),
            timestamps: HashMap::default(),
        }
    }
}

impl std::hash::Hash for RawCache {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let mut vec: Vec<_> = self.all_txs.keys().collect();
        vec.sort();
        vec.hash(state);

        let mut vec: Vec<_> = self.paths.iter().collect();
        vec.sort();
        vec.hash(state);

        // We don't hash the blinding public key for backward compatibility reasons
        let mut vec: Vec<_> = self.scripts.iter().map(|(k, v)| (k, &v.0)).collect();
        vec.sort();
        vec.hash(state);

        let mut vec: Vec<_> = self.heights.iter().collect();
        vec.sort();
        vec.hash(state);

        let mut vec: Vec<_> = self.unblinded.iter().collect();
        vec.sort_by_key(|kv| kv.0);
        vec.hash(state);

        self.tip.hash(state);

        let mut vec: Vec<_> = self.timestamps.iter().collect();
        vec.sort();
        vec.hash(state);

        self.last_unused_external
            .load(Ordering::Relaxed)
            .hash(state);

        self.last_unused_internal
            .load(Ordering::Relaxed)
            .hash(state);
    }
}

#[derive(Default, Debug)]
pub struct ScriptBatch {
    pub cached: bool,
    pub value: Vec<(Script, (Chain, ChildNumber, BlindingPublicKey))>,
}

impl RawCache {
    pub fn get_script_batch(
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

    pub(crate) fn get_or_derive(
        &self,
        ext_int: Chain,
        child: ChildNumber,
        descriptor: &ConfidentialDescriptor<DescriptorPublicKey>,
    ) -> Result<(Script, BlindingPublicKey, bool), Error> {
        let opt_script = self.scripts.get(&(ext_int, child));
        let (script, blinding_pubkey, cached) = match opt_script {
            Some((script, blinding_pubkey)) => (script.clone(), *blinding_pubkey, true),
            None => {
                let (script, blinding_pubkey) =
                    crate::wollet::derive_script_and_blinding_key(descriptor, child)?;
                (script, blinding_pubkey, false)
            }
        };
        Ok((script, blinding_pubkey, cached))
    }

    pub fn spent(&self) -> Result<HashSet<OutPoint>, Error> {
        Ok(self
            .all_txs
            .values()
            .flat_map(|tx| tx.input.iter())
            .map(|i| i.previous_output)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::{cache::RawCache, WolletDescriptor};
    use elements::{Address, AddressParams, Txid};
    use elements_miniscript::ConfidentialDescriptor;
    use std::{
        collections::hash_map::DefaultHasher,
        convert::TryInto,
        hash::{Hash, Hasher},
        str::FromStr,
    };
    use tempfile::TempDir;

    #[test]
    fn test_address_derivation() {
        let tempdir = TempDir::new().unwrap();
        let mut dir = tempdir.path().to_path_buf();
        dir.push("store");
        let xpub = "tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        let master_blinding_key =
            "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
        let checksum = "8w7cjcha";
        let desc_str = format!("ct(slip77({master_blinding_key}),elwpkh({xpub}/*))#{checksum}");
        let desc = ConfidentialDescriptor::<_>::from_str(&desc_str).unwrap();
        let desc: WolletDescriptor = desc.try_into().unwrap();
        let addr1 = desc.address(0, &AddressParams::LIQUID_TESTNET).unwrap();

        let cache = RawCache::default();

        let x = cache
            .get_script_batch(0, &desc.as_single_descriptors().unwrap()[0])
            .unwrap();
        assert_eq!(format!("{:?}", x.value[0]), "(Script(OP_0 OP_PUSHBYTES_20 d11ef9e68385138627b09d52d6fe12662d049224), (External, Normal { index: 0 }, PublicKey(0525054b498a69342d90750ed5e8f91cb6fb4da48735fd7011fdbcfc0e8edee1f0a30ed1e5c1d730e281b73f70f02dec2cbe20d0ac864d3d3d6942a02d66c6e3)))");
        assert_ne!(x.value[0], x.value[1]);
        let addr2 = Address::from_script(
            &x.value[0].0,
            Some(x.value[0].1 .2),
            &AddressParams::LIQUID_TESTNET,
        )
        .unwrap();
        assert_eq!(addr1, addr2)
    }

    #[test]
    fn test_cache_hash() {
        let mut cache = RawCache::default();
        let mut hasher = DefaultHasher::new();
        cache.hash(&mut hasher);
        assert_eq!(11565483422739161174, hasher.finish());

        cache
            .heights
            .insert(<Txid as elements::hashes::Hash>::all_zeros(), None);
        let mut hasher = DefaultHasher::new();
        cache.hash(&mut hasher);
        assert_eq!(12004253425667158821, hasher.finish());

        // TODO test other fields change the hash
    }
}
