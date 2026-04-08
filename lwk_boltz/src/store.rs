use std::sync::Arc;

use lwk_common::EncryptedStore;
use lwk_wollet::bitcoin::bip32::Xpub;
use lwk_wollet::hashes::{sha256t_hash_newtype, Hash};

use crate::error;
pub use lwk_common::DynStore;

sha256t_hash_newtype! {
    /// The tag of the hash for encryption key derivation
    pub struct BoltzEncryptionKeyTag = hash_str("LWK-Boltz-Encryption-Key/1.0");

    /// A tagged hash to generate the key for encryption in the Boltz store
    #[hash_newtype(forward)]
    pub struct BoltzEncryptionKeyHash(_);
}

/// Derive the 32-byte encryption key from an xpub.
///
/// The key is derived by hashing the xpub string bytes with a tagged hash.
pub fn key_bytes_from_xpub(xpub: &Xpub) -> [u8; 32] {
    BoltzEncryptionKeyHash::hash(xpub.to_string().as_bytes()).to_byte_array()
}

/// Wrap a store with key-and-value encryption derived from `xpub`.
///
/// The returned store encrypts all keys (deterministic nonce) and values (random nonce)
/// transparently, so callers can use plain string keys and unencrypted values.
pub fn encrypted_store_from_xpub(store: Arc<dyn DynStore>, xpub: &Xpub) -> Arc<dyn DynStore> {
    let key_bytes = key_bytes_from_xpub(xpub);
    Arc::new(EncryptedStore::new_with_key_encryption(store, key_bytes))
}

/// Store keys for Boltz swap persistence.
///
/// Encryption is handled transparently by the [`EncryptedStore`] wrapper; callers work
/// with plain keys and values.
pub mod store_keys {
    use super::*;

    /// Key name for the list of pending swap IDs
    const PENDING_SWAPS_KEY: &str = "boltz:pending_swaps";

    /// Key name for the list of completed swap IDs
    const COMPLETED_SWAPS_KEY: &str = "boltz:completed_swaps";

    /// Generate the key for a specific swap's data
    fn swap_data_key(swap_id: &str) -> String {
        format!("boltz:swap:{swap_id}")
    }

    /// Read the pending swaps list from the store
    ///
    /// Returns an empty Vec if the key doesn't exist, propagates errors on store
    /// access failure or deserialization failure.
    pub fn get_pending_swaps(store: &dyn DynStore) -> Result<Vec<String>, error::Error> {
        store
            .get(PENDING_SWAPS_KEY)
            .map_err(error::Error::Store)?
            .map(|data| serde_json::from_slice(&data).map_err(error::Error::from))
            .transpose()?
            .map_or_else(|| Ok(Vec::new()), Ok)
    }

    /// Read the completed swaps list from the store
    ///
    /// Returns an empty Vec if the key doesn't exist, propagates errors on store
    /// access failure or deserialization failure.
    pub fn get_completed_swaps(store: &dyn DynStore) -> Result<Vec<String>, error::Error> {
        store
            .get(COMPLETED_SWAPS_KEY)
            .map_err(error::Error::Store)?
            .map(|data| serde_json::from_slice(&data).map_err(error::Error::from))
            .transpose()?
            .map_or_else(|| Ok(Vec::new()), Ok)
    }

    /// Write the pending swaps list to the store
    pub fn set_pending_swaps(store: &dyn DynStore, swaps: &[String]) -> Result<(), error::Error> {
        let value = serde_json::to_vec(swaps)?;
        store
            .put(PENDING_SWAPS_KEY, &value)
            .map_err(error::Error::Store)
    }

    /// Write the completed swaps list to the store
    pub fn set_completed_swaps(store: &dyn DynStore, swaps: &[String]) -> Result<(), error::Error> {
        let value = serde_json::to_vec(swaps)?;
        store
            .put(COMPLETED_SWAPS_KEY, &value)
            .map_err(error::Error::Store)
    }

    /// Get swap data from the store
    pub fn get_swap_data(
        store: &dyn DynStore,
        swap_id: &str,
    ) -> Result<Option<Vec<u8>>, error::Error> {
        store
            .get(&swap_data_key(swap_id))
            .map_err(error::Error::Store)
    }

    /// Set swap data in the store
    pub fn set_swap_data(
        store: &dyn DynStore,
        swap_id: &str,
        data: &[u8],
    ) -> Result<(), error::Error> {
        store
            .put(&swap_data_key(swap_id), data)
            .map_err(error::Error::Store)
    }

    /// Remove swap data from the store
    pub fn remove_swap_data(store: &dyn DynStore, swap_id: &str) -> Result<(), error::Error> {
        store
            .remove(&swap_data_key(swap_id))
            .map_err(error::Error::Store)
    }
}

/// Trait for swap response types that support persistence.
pub trait SwapPersistence {
    /// Serialize the swap data to a JSON string
    fn serialize(&self) -> Result<String, error::Error>;

    /// Get the swap ID
    fn swap_id(&self) -> &str;

    /// Get the store, if configured
    fn store(&self) -> Option<Arc<dyn DynStore>>;

    /// Persist swap data to the store
    fn persist(&self) -> Result<(), error::Error> {
        if let Some(store) = self.store() {
            let data = self.serialize()?;
            store_keys::set_swap_data(store.as_ref(), self.swap_id(), data.as_bytes())?;
            log::debug!("Persisted swap data for {}", self.swap_id());
        }
        Ok(())
    }

    /// Persist swap data and add to pending swaps list
    fn persist_and_add_to_pending(&self) -> Result<(), error::Error> {
        if let Some(store) = self.store() {
            // Persist the swap data
            self.persist()?;
            // Add to pending list
            let mut pending = store_keys::get_pending_swaps(store.as_ref())?;
            let swap_id = self.swap_id().to_string();
            if !pending.contains(&swap_id) {
                pending.push(swap_id.clone());
                store_keys::set_pending_swaps(store.as_ref(), &pending)?;
                log::debug!("Added swap {swap_id} to pending list");
            }
        }
        Ok(())
    }

    /// Move swap from pending to completed list
    fn move_to_completed(&self) -> Result<(), error::Error> {
        if let Some(store) = self.store() {
            let swap_id = self.swap_id().to_string();

            // Remove from pending list
            let mut pending = store_keys::get_pending_swaps(store.as_ref())?;
            pending.retain(|id| id != &swap_id);
            store_keys::set_pending_swaps(store.as_ref(), &pending)?;

            // Add to completed list
            let mut completed = store_keys::get_completed_swaps(store.as_ref())?;
            if !completed.contains(&swap_id) {
                completed.push(swap_id.clone());
                store_keys::set_completed_swaps(store.as_ref(), &completed)?;
            }

            log::debug!("Moved swap {swap_id} to completed list");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bip39::Mnemonic;
    use lwk_common::MemoryStore;
    use lwk_wollet::bitcoin::NetworkKind;

    fn test_mnemonic() -> Mnemonic {
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            .parse()
            .unwrap()
    }

    fn test_xpub() -> Xpub {
        crate::derive_xpub_from_mnemonic(&test_mnemonic(), NetworkKind::Test).unwrap()
    }

    fn test_store(xpub: &Xpub) -> Arc<dyn DynStore> {
        encrypted_store_from_xpub(Arc::new(MemoryStore::new()), xpub)
    }

    #[test]
    fn test_key_encryption_is_deterministic() {
        let xpub = test_xpub();
        let inner = Arc::new(MemoryStore::new());
        let store1 = encrypted_store_from_xpub(inner.clone() as Arc<dyn DynStore>, &xpub);
        let store2 = encrypted_store_from_xpub(inner.clone() as Arc<dyn DynStore>, &xpub);

        // Both stores encrypt the same key to the same ciphertext (deterministic nonce)
        let value = b"some value";
        store1.put("boltz:pending_swaps", value).unwrap();
        let read_back = store2.get("boltz:pending_swaps").unwrap();
        // store2 can read what store1 wrote because they share xpub
        assert_eq!(read_back, Some(value.to_vec()));
    }

    #[test]
    fn test_store_roundtrip() {
        let xpub = test_xpub();
        let store = test_store(&xpub);

        let swaps = vec!["swap1".to_string(), "swap2".to_string()];
        store_keys::set_pending_swaps(store.as_ref(), &swaps).unwrap();
        let loaded = store_keys::get_pending_swaps(store.as_ref()).unwrap();
        assert_eq!(swaps, loaded);

        let swap_data = b"swap json data here";
        store_keys::set_swap_data(store.as_ref(), "swap1", swap_data).unwrap();
        let loaded_data = store_keys::get_swap_data(store.as_ref(), "swap1")
            .unwrap()
            .unwrap();
        assert_eq!(swap_data.to_vec(), loaded_data);
    }

    #[test]
    fn test_different_xpubs_cannot_read_each_other() {
        let mnemonic2: Mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong"
            .parse()
            .unwrap();
        let xpub1 = test_xpub();
        let xpub2 = crate::derive_xpub_from_mnemonic(&mnemonic2, NetworkKind::Test).unwrap();

        let inner = Arc::new(MemoryStore::new());
        let store1 = encrypted_store_from_xpub(inner.clone() as Arc<dyn DynStore>, &xpub1);
        let store2 = encrypted_store_from_xpub(inner.clone() as Arc<dyn DynStore>, &xpub2);

        let swaps = vec!["swap1".to_string()];
        store_keys::set_pending_swaps(store1.as_ref(), &swaps).unwrap();

        // store2 (different xpub) cannot decrypt store1's data
        let pending_swaps1 = store_keys::get_pending_swaps(store1.as_ref()).unwrap();
        assert_eq!(pending_swaps1.len(), 1);
        let pending_swaps2 = store_keys::get_pending_swaps(store2.as_ref()).unwrap();
        assert_eq!(pending_swaps2.len(), 0);
    }

    #[test]
    fn test_boltz_encryption_key_hash_empty_input_regression() {
        let got = BoltzEncryptionKeyHash::hash(b"").to_string();
        let exp = "107900f3750784c733ae53cd00433ec0c10c36517a2a68a904b749d7f98d06e0";
        assert_eq!(got, exp);
    }

    #[test]
    #[ignore = "requires internet connection"]
    fn test_boltz_store_fs() {
        use crate::{clients::EsploraClient, AnyClient, BoltzSessionBuilder};
        use lwk_common::FileStore;
        use lwk_signer::SwSigner;
        use lwk_wollet::clients::asyncr::EsploraClientBuilder;
        use lwk_wollet::ElementsNetwork;
        use std::path::PathBuf;

        let network = ElementsNetwork::Liquid;

        let url = "https://waterfalls.liquidwebwallet.org/liquid/api";
        let client = EsploraClientBuilder::new(url, network)
            .waterfalls(true)
            .build()
            .unwrap();
        let client = EsploraClient::from_client(Arc::new(client), network);
        let client = AnyClient::Esplora(Arc::new(client));

        let mnemonic =
            "craft travel attitude order useful orient venue true double motor enable already";
        let is_mainnet = true;
        let signer = SwSigner::new(mnemonic, is_mainnet).unwrap();
        let index = 26589;
        let word_count = 12;
        let mnemonic_ln = signer.derive_bip85_mnemonic(index, word_count).unwrap();

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("store-swaps");
        let store = FileStore::new(path).unwrap();
        let store = Arc::new(store);

        let session = BoltzSessionBuilder::new(network, client)
            .mnemonic(mnemonic_ln)
            .store(store)
            .build_blocking()
            .unwrap();
        let pending_swaps = session.pending_swap_ids().unwrap();
        assert_eq!(pending_swaps.len(), 1);
        assert_eq!(pending_swaps[0], "xVqfzgPQ7NWt");
        let completed_swaps = session.completed_swap_ids().unwrap();
        assert_eq!(completed_swaps.len(), 0);
    }
}
