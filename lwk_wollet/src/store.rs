use crate::descriptor::Chain;
use crate::elements::{BlockHash, OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::hashes::Hash;
use crate::Error;
use elements::bitcoin::bip32::ChildNumber;
use elements_miniscript::{Descriptor, DescriptorPublicKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};

pub const BATCH_SIZE: u32 = 20;
pub type Height = u32;
pub type Timestamp = u32;

/// RawCache is a persisted and encrypted cache of wallet data, contains stuff like wallet transactions
/// It is fully reconstructable from xpub and data from electrum server (plus master blinding for elements)
#[derive(Serialize, Deserialize)]
pub struct RawCache {
    /// contains all my tx and all prevouts
    pub all_txs: HashMap<Txid, Transaction>,

    /// contains all my script up to an empty batch of BATCHSIZE
    pub paths: HashMap<Script, (Chain, ChildNumber)>,

    /// inverse of `paths`
    pub scripts: HashMap<(Chain, ChildNumber), Script>,

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
        let mut vec: Vec<_> = self.all_txs.iter().collect();
        vec.sort();
        vec.hash(state);

        let mut vec: Vec<_> = self.paths.iter().collect();
        vec.sort();
        vec.hash(state);

        let mut vec: Vec<_> = self.scripts.iter().collect();
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

#[derive(Default, Hash)]
pub struct Store {
    pub cache: RawCache,
}

#[derive(Default, Debug)]
pub struct ScriptBatch {
    pub cached: bool,
    pub value: Vec<(Script, (Chain, ChildNumber))>,
}

impl Store {
    pub fn get_script_batch(
        &self,
        batch: u32,
        descriptor: &Descriptor<DescriptorPublicKey>, // non confidential (we need only script_pubkey), non multipath (we need to be able to derive with index)
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
            let opt_script = self.cache.scripts.get(&(ext_int, child));
            let script = match opt_script {
                Some(script) => script.clone(),
                None => {
                    result.cached = false;
                    descriptor.at_derivation_index(j)?.script_pubkey()
                }
            };
            result.value.push((script, (ext_int, child)));
        }

        Ok(result)
    }

    pub fn spent(&self) -> Result<HashSet<OutPoint>, Error> {
        let mut result = HashSet::new();
        for tx in self.cache.all_txs.values() {
            let outpoints: Vec<OutPoint> = tx.input.iter().map(|i| i.previous_output).collect();
            result.extend(outpoints.into_iter());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::{store::Store, WolletDescriptor};
    use elements::Txid;
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
        let desc_str = format!(
            "ct(slip77({}),elwpkh({}/*))#{}",
            master_blinding_key, xpub, checksum
        );
        let desc = ConfidentialDescriptor::<_>::from_str(&desc_str).unwrap();
        let desc: WolletDescriptor = desc.try_into().unwrap();

        let store = Store::default();

        let x = store
            .get_script_batch(0, &desc.as_ref().descriptor)
            .unwrap();
        assert_eq!(format!("{:?}", x.value[0]), "(Script(OP_0 OP_PUSHBYTES_20 d11ef9e68385138627b09d52d6fe12662d049224), (External, Normal { index: 0 }))");
        assert_ne!(x.value[0], x.value[1]);
    }

    #[test]
    fn test_store_hash() {
        let mut store = Store::default();
        let mut hasher = DefaultHasher::new();
        store.hash(&mut hasher);
        assert_eq!(11565483422739161174, hasher.finish());

        store
            .cache
            .heights
            .insert(<Txid as elements::hashes::Hash>::all_zeros(), None);
        let mut hasher = DefaultHasher::new();
        store.hash(&mut hasher);
        assert_eq!(12004253425667158821, hasher.finish());

        // TODO test other fields change the hash
    }
}
