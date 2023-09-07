use crate::scripts::p2shwpkh_script;
use crate::Error;
use aes_gcm_siv::aead::{generic_array::GenericArray, AeadInPlace, NewAead};
use aes_gcm_siv::Aes256GcmSiv;
use elements::bitcoin::hashes::sha256;
use elements::bitcoin::hashes::Hash;
use elements::bitcoin::secp256k1::{All, Secp256k1};
use elements::bitcoin::util::bip32::{ChildNumber, DerivationPath, ExtendedPubKey};
use elements::OutPoint;
use elements::{BlockHash, Script, Txid};
use log::warn;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, RwLock};

pub const BATCH_SIZE: u32 = 20;

pub type Store = Arc<RwLock<StoreMeta>>;

pub fn new_store<P: AsRef<Path>>(path: P, xpub: ExtendedPubKey) -> Result<Store, Error> {
    Ok(Arc::new(RwLock::new(StoreMeta::new(&path, xpub)?)))
}

/// RawCache is a persisted and encrypted cache of wallet data, contains stuff like wallet transactions
/// It is fully reconstructable from xpub and data from electrum server (plus master blinding for elements)
#[derive(Serialize, Deserialize)]
pub struct RawCache {
    /// contains all my tx and all prevouts
    pub all_txs: HashMap<Txid, elements::Transaction>,

    /// contains all my script up to an empty batch of BATCHSIZE
    pub paths: HashMap<Script, DerivationPath>,

    /// inverse of `paths`
    pub scripts: HashMap<DerivationPath, Script>, // TODO use DerivationPath once Hash gets merged

    /// contains only my wallet txs with the relative heights (None if unconfirmed)
    pub heights: HashMap<Txid, Option<u32>>,

    /// unblinded values (only for liquid)
    pub unblinded: HashMap<OutPoint, elements::TxOutSecrets>,

    /// height and hash of tip of the blockchain
    pub tip: (u32, BlockHash),

    /// max used indexes for external derivation /0/* and internal derivation /1/* (change)
    pub indexes: Indexes,
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
            indexes: Indexes::default(),
        }
    }
}

pub struct StoreMeta {
    pub cache: RawCache,
    secp: Secp256k1<All>,
    path: PathBuf,
    cipher: Aes256GcmSiv,
    first_deriv: [ExtendedPubKey; 2],
}

impl Drop for StoreMeta {
    fn drop(&mut self) {
        self.flush().unwrap();
    }
}

#[derive(Debug, PartialEq, Eq, Default, Clone, Serialize, Deserialize)]
pub struct Indexes {
    pub external: u32, // m/0/*
    pub internal: u32, // m/1/*
}

#[derive(Default)]
pub struct ScriptBatch {
    pub cached: bool,
    pub value: Vec<(Script, DerivationPath)>,
}

impl RawCache {
    /// create a new RawCache, loading data from a file if any and if there is no error in reading
    /// errors such as corrupted file or model change in the db, result in a empty store that will be repopulated
    fn new<P: AsRef<Path>>(path: P, cipher: &Aes256GcmSiv) -> Self {
        Self::try_new(path, cipher).unwrap_or_else(|e| {
            warn!("Initialize cache as default {:?}", e);
            Default::default()
        })
    }

    fn try_new<P: AsRef<Path>>(path: P, cipher: &Aes256GcmSiv) -> Result<Self, Error> {
        let decrypted = load_decrypt("cache", path, cipher)?;
        let store = serde_cbor::from_slice(&decrypted)?;
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

impl StoreMeta {
    pub fn new<P: AsRef<Path>>(path: P, xpub: ExtendedPubKey) -> Result<StoreMeta, Error> {
        let mut enc_key_data = vec![];
        enc_key_data.extend(&xpub.public_key.serialize());
        enc_key_data.extend(&xpub.chain_code.to_bytes());
        enc_key_data.extend(&xpub.network.magic().to_be_bytes());
        let key_bytes = sha256::Hash::hash(&enc_key_data).into_inner();
        let key = GenericArray::from_slice(&key_bytes);
        let cipher = Aes256GcmSiv::new(&key);
        let cache = RawCache::new(path.as_ref(), &cipher);
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        let secp = Secp256k1::new();

        let first_deriv = [
            xpub.derive_pub(&secp, &[ChildNumber::from(0)])?,
            xpub.derive_pub(&secp, &[ChildNumber::from(1)])?,
        ];

        Ok(StoreMeta {
            cache,
            cipher,
            secp,
            path,
            first_deriv,
        })
    }

    fn flush_serializable<T: serde::Serialize>(&self, name: &str, value: &T) -> Result<(), Error> {
        let mut nonce_bytes = [0u8; 12];
        thread_rng().fill(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);
        let mut plaintext = serde_cbor::to_vec(value)?;

        self.cipher.encrypt_in_place(nonce, b"", &mut plaintext)?;
        let ciphertext = plaintext;

        let mut store_path = self.path.clone();
        store_path.push(name);
        //TODO should avoid rewriting if not changed? it involves saving plaintext (or struct hash)
        // in the front of the file
        let mut file = File::create(&store_path)?;
        file.write(&nonce_bytes)?;
        file.write(&ciphertext)?;
        Ok(())
    }

    fn flush_cache(&self) -> Result<(), Error> {
        self.flush_serializable("cache", &self.cache)?;
        Ok(())
    }

    pub fn flush(&self) -> Result<(), Error> {
        self.flush_cache()?;
        Ok(())
    }

    pub fn get_script_batch(&self, int_or_ext: u32, batch: u32) -> Result<ScriptBatch, Error> {
        let mut result = ScriptBatch::default();
        result.cached = true;

        //TODO cache m/0 and m/1
        let first_deriv = &self.first_deriv[int_or_ext as usize];

        let start = batch * BATCH_SIZE;
        let end = start + BATCH_SIZE;
        for j in start..end {
            let path = DerivationPath::from_str(&format!("m/{}/{}", int_or_ext, j))?;
            let opt_script = self.cache.scripts.get(&path);
            let script = match opt_script {
                Some(script) => script.clone(),
                None => {
                    result.cached = false;
                    let second_path = [ChildNumber::from(j)];
                    let second_deriv = first_deriv.derive_pub(&self.secp, &second_path)?;
                    p2shwpkh_script(&second_deriv.to_pub())
                }
            };
            result.value.push((script, path));
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
    use crate::store::StoreMeta;
    use elements::bitcoin::hashes::hex::FromHex;
    use elements::bitcoin::util::bip32::ExtendedPubKey;
    use elements::Txid;
    use std::str::FromStr;
    use tempdir::TempDir;

    #[test]
    fn test_db_roundtrip() {
        let mut dir = TempDir::new("unit_test").unwrap().into_path();
        dir.push("store");
        let xpub = ExtendedPubKey::from_str("tpubD6NzVbkrYhZ4YfG9CySHqKHFbaLcD7hSDyqRUtCmMKNim5fkiJtTnFeqKsRHMHSK5ddFrhqRr3Ghv1JtuWkBzikuBqKu1xCpjQ9YxoPGgqU").unwrap();
        let txid =
            Txid::from_hex("f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16")
                .unwrap();

        let mut store = StoreMeta::new(&dir, xpub).unwrap();
        store.cache.heights.insert(txid, Some(1));
        drop(store);

        let store = StoreMeta::new(&dir, xpub).unwrap();
        assert_eq!(store.cache.heights.get(&txid), Some(&Some(1)));
    }
}
