use crate::descriptor::Chain;
use crate::elements::{BlockHash, OutPoint, Script, Transaction, TxOutSecrets, Txid};
use crate::hashes::{sha256, Hash};
use crate::{Error, WolletDescriptor};
use aes_gcm_siv::aead::generic_array::GenericArray;
use aes_gcm_siv::aead::{AeadInPlace, NewAead};
use aes_gcm_siv::Aes256GcmSiv;
use electrum_client::bitcoin::bip32::ChildNumber;
use elements_miniscript::{Descriptor, DescriptorPublicKey};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;

pub const BATCH_SIZE: u32 = 20;
pub type Height = u32;
pub type Timestamp = u32;

pub fn new_store<P: AsRef<Path>>(path: P, desc: &WolletDescriptor) -> Result<Store, Error> {
    Store::new(Some(&path), desc)
}

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

pub struct Store {
    pub cache: RawCache,
    persister: Option<Persister>,
}
pub struct Persister {
    pub path: PathBuf,
    pub cipher: Aes256GcmSiv,
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[derive(Default, Debug)]
pub struct ScriptBatch {
    pub cached: bool,
    pub value: Vec<(Script, (Chain, ChildNumber))>,
}

impl RawCache {
    /// create a new RawCache, loading data from a file if any and if there is no error in reading
    /// errors such as corrupted file or model change in the db, result in a empty store that will be repopulated
    fn new<P: AsRef<Path>>(path: P, cipher: &Aes256GcmSiv) -> Self {
        Self::try_new(path, cipher).unwrap_or_else(|e| {
            tracing::warn!("Initialize cache as default {:?}", e);
            Default::default()
        })
    }

    fn try_new<P: AsRef<Path>>(path: P, cipher: &Aes256GcmSiv) -> Result<Self, Error> {
        let decrypted = load_decrypt("cache", path, cipher)?;
        let store = serde_cbor::from_reader(&decrypted[..])?;
        Ok(store)
    }
}

fn load_decrypt<P: AsRef<Path>>(
    name: &str,
    path: P,
    cipher: &Aes256GcmSiv,
) -> Result<Vec<u8>, Error> {
    let mut store_path = PathBuf::from(path.as_ref());
    store_path.push(name);
    if !store_path.exists() {
        return Err(Error::Generic(format!("{:?} do not exist", store_path)));
    }
    let mut file = File::open(&store_path)?;
    let mut nonce_bytes = [0u8; 12];
    file.read_exact(&mut nonce_bytes)?;
    let nonce = GenericArray::from_slice(&nonce_bytes);
    let mut ciphertext = vec![];
    file.read_to_end(&mut ciphertext)?;

    cipher.decrypt_in_place(nonce, b"", &mut ciphertext)?;
    let plaintext = ciphertext;

    Ok(plaintext)
}

impl Store {
    pub fn new<P: AsRef<Path>>(
        path: Option<P>,
        descriptor: &WolletDescriptor,
    ) -> Result<Store, Error> {
        /*
        let mut enc_key_data = vec![];
        enc_key_data.extend(&xpub.public_key.serialize());
        enc_key_data.extend(&xpub.chain_code.to_bytes());
        enc_key_data.extend(&xpub.network.magic().to_bytes());
        let key_bytes = sha256::Hash::hash(&enc_key_data).to_byte_array();
        */

        match path.as_ref() {
            Some(path) => {
                let key_bytes =
                    sha256::Hash::hash(descriptor.to_string().as_bytes()).to_byte_array();
                let key = GenericArray::from_slice(&key_bytes);
                let cipher = Aes256GcmSiv::new(key);
                let cache = RawCache::new(path, &cipher);
                let path = path.as_ref().to_path_buf();
                if !path.exists() {
                    std::fs::create_dir_all(&path)?;
                }

                Ok(Store {
                    cache,
                    persister: Some(Persister { path, cipher }),
                })
            }
            None => Ok(Store {
                cache: RawCache::default(),
                persister: None,
            }),
        }
    }

    fn flush_serializable<T: serde::Serialize>(&self, name: &str, value: &T) -> Result<(), Error> {
        if let Some(Persister { cipher, path }) = self.persister.as_ref() {
            let mut nonce_bytes = [0u8; 12];
            thread_rng().fill(&mut nonce_bytes);
            let nonce = GenericArray::from_slice(&nonce_bytes);
            let mut plaintext = serde_cbor::to_vec(value)?;

            cipher.encrypt_in_place(nonce, b"", &mut plaintext)?;
            let ciphertext = plaintext;

            let mut store_path = path.clone();
            store_path.push(name);
            //TODO should avoid rewriting if not changed? it involves saving plaintext (or struct hash)
            // in the front of the file
            let mut file = File::create(&store_path)?;
            file.write_all(&nonce_bytes)?;
            file.write_all(&ciphertext)?;
        }
        Ok(())
    }

    pub fn flush(&self) -> Result<(), Error> {
        self.flush_serializable("cache", &self.cache)?;
        Ok(())
    }

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
    use std::{convert::TryInto, str::FromStr};
    use tempfile::TempDir;

    #[test]
    fn test_db_roundtrip() {
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
        let txid =
            Txid::from_str("f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16")
                .unwrap();

        let mut store = Store::new(Some(&dir), &desc.clone().try_into().unwrap()).unwrap();
        store.cache.heights.insert(txid, Some(1));
        drop(store);

        let store = Store::new(Some(&dir), &desc.try_into().unwrap()).unwrap();
        assert_eq!(store.cache.heights.get(&txid), Some(&Some(1)));
    }

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

        let store = Store::new(Some(&dir), &desc).unwrap();

        let x = store
            .get_script_batch(0, &desc.as_ref().descriptor)
            .unwrap();
        assert_eq!(format!("{:?}", x.value[0]), "(Script(OP_0 OP_PUSHBYTES_20 d11ef9e68385138627b09d52d6fe12662d049224), (External, Normal { index: 0 }))");
        assert_ne!(x.value[0], x.value[1]);
    }
}
