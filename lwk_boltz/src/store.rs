use std::sync::Arc;

#[allow(deprecated)]
use aes_gcm_siv::aead::generic_array::GenericArray;
use aes_gcm_siv::aead::AeadMutInPlace;
use aes_gcm_siv::{Aes256GcmSiv, KeyInit};
use lwk_wollet::bitcoin::bip32::Xpub;
use lwk_wollet::hashes::hex::{DisplayHex, FromHex};
use lwk_wollet::hashes::{sha256, sha256t_hash_newtype, Hash};
use rand::{thread_rng, Rng};

use crate::error;
pub use lwk_common::DynStore;

sha256t_hash_newtype! {
    /// The tag of the hash for encryption key derivation
    pub struct BoltzEncryptionKeyTag = hash_str("LWK-Boltz-Encryption-Key/1.0");

    /// A tagged hash to generate the key for encryption in the Boltz store
    #[hash_newtype(forward)]
    pub struct BoltzEncryptionKeyHash(_);

    /// The tag of the hash for store prefix derivation
    pub struct BoltzStorePrefixTag = hash_str("LWK-Boltz-Store-Prefix/1.0");

    /// A tagged hash to generate the store key prefix from mnemonic
    #[hash_newtype(forward)]
    pub struct BoltzStorePrefixHash(_);
}

/// Create a cipher from an xpub for encrypting Boltz store data.
///
/// The encryption key is derived by hashing the xpub string bytes with a tagged hash.
#[allow(deprecated)]
pub fn cipher_from_xpub(xpub: &Xpub) -> Aes256GcmSiv {
    let xpub_string = xpub.to_string();
    let key_bytes = BoltzEncryptionKeyHash::hash(xpub_string.as_bytes()).to_byte_array();
    let key = GenericArray::from_slice(&key_bytes);
    Aes256GcmSiv::new(key)
}

/// Encrypt a store key deterministically.
///
/// Uses a nonce derived from the key's SHA256 hash to ensure the same key always
/// produces the same encrypted output, enabling GET operations.
#[allow(deprecated)]
pub fn encrypt_key(cipher: &mut Aes256GcmSiv, key: &str) -> Result<String, error::Error> {
    // Derive nonce from key hash (first 12 bytes of SHA256)
    let hash = sha256::Hash::hash(key.as_bytes());
    let nonce_bytes: [u8; 12] = hash.as_byte_array()[..12]
        .try_into()
        .expect("12 bytes from 32 byte hash");
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let mut buffer = key.as_bytes().to_vec();
    cipher.encrypt_in_place(nonce, b"", &mut buffer)?;

    Ok(buffer.to_lower_hex_string())
}

/// Decrypt a store key.
///
/// Reverses the deterministic encryption using the same derived nonce.
#[allow(deprecated)]
#[allow(dead_code)] // This function exists for completeness but isn't used in normal operation
pub fn decrypt_key(
    _cipher: &mut Aes256GcmSiv,
    encrypted_hex: &str,
) -> Result<String, error::Error> {
    let _buffer = Vec::<u8>::from_hex(encrypted_hex)
        .map_err(|e| error::Error::Encryption(format!("invalid hex: {e}")))?;

    // We need the original plaintext to derive the nonce, but we don't have it.
    // This is a fundamental issue with the deterministic key encryption approach.
    // Instead, we'll iterate through all possible keys when decrypting.
    // However, for practical use, we typically know what keys we're looking for.
    //
    // For now, return an error - decryption of keys isn't needed for normal operations
    // since we always derive the encrypted key from the plaintext key.
    Err(error::Error::Encryption(
        "Key decryption not supported - use encrypt_key with the plaintext key instead".into(),
    ))
}

/// Encrypt a value with a random nonce.
///
/// The nonce is prepended to the ciphertext for later decryption.
#[allow(deprecated)]
pub fn encrypt_value(cipher: &mut Aes256GcmSiv, value: &[u8]) -> Result<Vec<u8>, error::Error> {
    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill(&mut nonce_bytes);
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let mut buffer = value.to_vec();
    cipher.encrypt_in_place(nonce, b"", &mut buffer)?;

    // Prepend nonce to ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend(buffer);
    Ok(result)
}

/// Decrypt a value that was encrypted with encrypt_value.
///
/// Extracts the nonce from the first 12 bytes of the ciphertext.
#[allow(deprecated)]
pub fn decrypt_value(cipher: &mut Aes256GcmSiv, data: &[u8]) -> Result<Vec<u8>, error::Error> {
    if data.len() < 12 {
        return Err(error::Error::Encryption(
            "Encrypted data too short - missing nonce".into(),
        ));
    }

    let nonce_bytes: [u8; 12] = data[..12]
        .try_into()
        .expect("12 bytes from slice of at least 12 bytes");
    let nonce = GenericArray::from_slice(&nonce_bytes);

    let mut buffer = data[12..].to_vec();
    cipher.decrypt_in_place(nonce, b"", &mut buffer)?;

    Ok(buffer)
}

/// Store keys for Boltz swap persistence with encryption support.
///
/// All keys and values are encrypted using the cipher derived from the mnemonic.
/// Keys use deterministic encryption (derived nonce) for GET operations.
/// We don't need a unique prefix for each keys because different mnemonics will have different ciphers.
/// Values use random nonces for stronger security.
pub mod store_keys {
    use super::*;

    /// Key name for the list of pending swap IDs
    const PENDING_SWAPS_KEY: &str = "boltz:pending_swaps";

    /// Key name for the list of completed swap IDs
    const COMPLETED_SWAPS_KEY: &str = "boltz:completed_swaps";

    /// Generate the key for the list of pending swap IDs
    fn pending_swaps_key() -> &'static str {
        PENDING_SWAPS_KEY
    }

    /// Generate the key for the list of completed swap IDs
    fn completed_swaps_key() -> &'static str {
        COMPLETED_SWAPS_KEY
    }

    /// Generate the key for a specific swap's data
    fn swap_data_key(swap_id: &str) -> String {
        format!("boltz:swap:{swap_id}")
    }

    /// Encrypt a key for storage
    fn encrypt_store_key(cipher: &mut Aes256GcmSiv, key: &str) -> Result<String, error::Error> {
        encrypt_key(cipher, key)
    }

    /// Read the pending swaps list from the store
    ///
    /// Returns an empty Vec if the key doesn't exist, propagates errors on store
    /// access failure or deserialization failure.
    pub fn get_pending_swaps(
        store: &dyn DynStore,
        cipher: &mut Aes256GcmSiv,
    ) -> Result<Vec<String>, error::Error> {
        let encrypted_key = encrypt_store_key(cipher, pending_swaps_key())?;
        store
            .get(&encrypted_key)
            .map_err(error::Error::Store)?
            .map(|data| {
                let decrypted = decrypt_value(cipher, &data)?;
                serde_json::from_slice(&decrypted).map_err(error::Error::from)
            })
            .transpose()?
            .map_or_else(|| Ok(Vec::new()), Ok)
    }

    /// Read the completed swaps list from the store
    ///
    /// Returns an empty Vec if the key doesn't exist, propagates errors on store
    /// access failure or deserialization failure.
    pub fn get_completed_swaps(
        store: &dyn DynStore,
        cipher: &mut Aes256GcmSiv,
    ) -> Result<Vec<String>, error::Error> {
        let encrypted_key = encrypt_store_key(cipher, completed_swaps_key())?;
        store
            .get(&encrypted_key)
            .map_err(error::Error::Store)?
            .map(|data| {
                let decrypted = decrypt_value(cipher, &data)?;
                serde_json::from_slice(&decrypted).map_err(error::Error::from)
            })
            .transpose()?
            .map_or_else(|| Ok(Vec::new()), Ok)
    }

    /// Write the pending swaps list to the store
    pub fn set_pending_swaps(
        store: &dyn DynStore,
        cipher: &mut Aes256GcmSiv,
        swaps: &[String],
    ) -> Result<(), error::Error> {
        let encrypted_key = encrypt_store_key(cipher, pending_swaps_key())?;
        let plaintext = serde_json::to_vec(swaps)?;
        let encrypted_value = encrypt_value(cipher, &plaintext)?;
        store
            .put(&encrypted_key, &encrypted_value)
            .map_err(error::Error::Store)
    }

    /// Write the completed swaps list to the store
    pub fn set_completed_swaps(
        store: &dyn DynStore,
        cipher: &mut Aes256GcmSiv,
        swaps: &[String],
    ) -> Result<(), error::Error> {
        let encrypted_key = encrypt_store_key(cipher, completed_swaps_key())?;
        let plaintext = serde_json::to_vec(swaps)?;
        let encrypted_value = encrypt_value(cipher, &plaintext)?;
        store
            .put(&encrypted_key, &encrypted_value)
            .map_err(error::Error::Store)
    }

    /// Get swap data from the store
    pub fn get_swap_data(
        store: &dyn DynStore,
        cipher: &mut Aes256GcmSiv,
        swap_id: &str,
    ) -> Result<Option<Vec<u8>>, error::Error> {
        let key = swap_data_key(swap_id);
        let encrypted_key = encrypt_store_key(cipher, &key)?;
        store
            .get(&encrypted_key)
            .map_err(error::Error::Store)?
            .map(|data| decrypt_value(cipher, &data))
            .transpose()
    }

    /// Set swap data in the store
    pub fn set_swap_data(
        store: &dyn DynStore,
        cipher: &mut Aes256GcmSiv,
        swap_id: &str,
        data: &[u8],
    ) -> Result<(), error::Error> {
        let key = swap_data_key(swap_id);
        let encrypted_key = encrypt_store_key(cipher, &key)?;
        let encrypted_value = encrypt_value(cipher, data)?;
        store
            .put(&encrypted_key, &encrypted_value)
            .map_err(error::Error::Store)
    }
}

/// Trait for swap response types that support persistence.
///
/// This trait provides the interface needed for persisting swap data to a store.
/// Implementors must provide serialization, swap ID access, store access, and cipher.
/// Default implementations are provided for persist operations.
pub trait SwapPersistence {
    /// Serialize the swap data to a JSON string
    fn serialize(&self) -> Result<String, error::Error>;

    /// Get the swap ID
    fn swap_id(&self) -> &str;

    /// Get a reference to the store, if configured
    fn store(&self) -> Option<&Arc<dyn DynStore>>;

    /// Get the cipher for encryption/decryption
    fn cipher(&self) -> Option<Aes256GcmSiv>;

    /// Persist swap data to the store
    fn persist(&self) -> Result<(), error::Error> {
        if let (Some(store), Some(mut cipher)) = (self.store(), self.cipher()) {
            let data = self.serialize()?;
            store_keys::set_swap_data(
                store.as_ref(),
                &mut cipher,
                self.swap_id(),
                data.as_bytes(),
            )?;
            log::debug!("Persisted swap data for {}", self.swap_id());
        }
        Ok(())
    }

    /// Persist swap data and add to pending swaps list
    fn persist_and_add_to_pending(&self) -> Result<(), error::Error> {
        if let (Some(store), Some(mut cipher)) = (self.store(), self.cipher()) {
            // Persist the swap data
            self.persist()?;

            // Add to pending list
            let mut pending = store_keys::get_pending_swaps(store.as_ref(), &mut cipher)?;

            let swap_id = self.swap_id().to_string();
            if !pending.contains(&swap_id) {
                pending.push(swap_id.clone());
                store_keys::set_pending_swaps(store.as_ref(), &mut cipher, &pending)?;
                log::debug!("Added swap {swap_id} to pending list");
            }
        }
        Ok(())
    }

    /// Move swap from pending to completed list
    fn move_to_completed(&self) -> Result<(), error::Error> {
        if let (Some(store), Some(mut cipher)) = (self.store(), self.cipher()) {
            let swap_id = self.swap_id().to_string();

            // Remove from pending list
            let mut pending = store_keys::get_pending_swaps(store.as_ref(), &mut cipher)?;
            pending.retain(|id| id != &swap_id);
            store_keys::set_pending_swaps(store.as_ref(), &mut cipher, &pending)?;

            // Add to completed list
            let mut completed = store_keys::get_completed_swaps(store.as_ref(), &mut cipher)?;
            if !completed.contains(&swap_id) {
                completed.push(swap_id.clone());
                store_keys::set_completed_swaps(store.as_ref(), &mut cipher, &completed)?;
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

    #[test]
    fn test_key_encryption_is_deterministic() {
        let xpub = test_xpub();
        let mut cipher1 = cipher_from_xpub(&xpub);
        let mut cipher2 = cipher_from_xpub(&xpub);

        let key = "boltz:pending_swaps";
        let encrypted1 = encrypt_key(&mut cipher1, key).unwrap();
        let encrypted2 = encrypt_key(&mut cipher2, key).unwrap();

        assert_eq!(
            encrypted1, encrypted2,
            "Key encryption should be deterministic"
        );
    }

    #[test]
    fn test_value_encryption_is_not_deterministic() {
        let xpub = test_xpub();
        let mut cipher1 = cipher_from_xpub(&xpub);
        let mut cipher2 = cipher_from_xpub(&xpub);

        let value = b"test value data";
        let encrypted1 = encrypt_value(&mut cipher1, value).unwrap();
        let encrypted2 = encrypt_value(&mut cipher2, value).unwrap();

        assert_ne!(
            encrypted1, encrypted2,
            "Value encryption should use random nonces"
        );
    }

    #[test]
    fn test_value_encryption_roundtrip() {
        let xpub = test_xpub();
        let mut cipher = cipher_from_xpub(&xpub);

        let original = b"test value with some data to encrypt";
        let encrypted = encrypt_value(&mut cipher, original).unwrap();

        // Need a fresh cipher for decryption
        let mut cipher = cipher_from_xpub(&xpub);
        let decrypted = decrypt_value(&mut cipher, &encrypted).unwrap();

        assert_eq!(original.to_vec(), decrypted);
    }

    #[test]
    fn test_store_roundtrip() {
        let store = MemoryStore::new();
        let xpub = test_xpub();
        let mut cipher = cipher_from_xpub(&xpub);

        // Test pending swaps
        let swaps = vec!["swap1".to_string(), "swap2".to_string()];
        store_keys::set_pending_swaps(&store, &mut cipher, &swaps).unwrap();

        let mut cipher = cipher_from_xpub(&xpub);
        let loaded = store_keys::get_pending_swaps(&store, &mut cipher).unwrap();
        assert_eq!(swaps, loaded);

        // Test swap data
        let mut cipher = cipher_from_xpub(&xpub);
        let swap_data = b"swap json data here";
        store_keys::set_swap_data(&store, &mut cipher, "swap1", swap_data).unwrap();

        let mut cipher = cipher_from_xpub(&xpub);
        let loaded_data = store_keys::get_swap_data(&store, &mut cipher, "swap1")
            .unwrap()
            .unwrap();
        assert_eq!(swap_data.to_vec(), loaded_data);
    }

    #[test]
    fn test_different_mnemonics_produce_different_ciphers() {
        let mnemonic1 = test_mnemonic();
        let mnemonic2: Mnemonic = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong"
            .parse()
            .unwrap();

        let xpub1 = crate::derive_xpub_from_mnemonic(&mnemonic1, NetworkKind::Test).unwrap();
        let xpub2 = crate::derive_xpub_from_mnemonic(&mnemonic2, NetworkKind::Test).unwrap();
        let mut cipher1 = cipher_from_xpub(&xpub1);
        let mut cipher2 = cipher_from_xpub(&xpub2);

        let key = "boltz:pending_swaps";
        let encrypted1 = encrypt_key(&mut cipher1, key).unwrap();
        let encrypted2 = encrypt_key(&mut cipher2, key).unwrap();

        assert_ne!(
            encrypted1, encrypted2,
            "Different mnemonics should produce different encrypted keys"
        );
    }
}
