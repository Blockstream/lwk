//! Generic key-value storage interface.
//!
//! This module defines the [`Store`] trait, which provides a simple key-value
//! storage abstraction. Implementations can back this with various storage backends
//! (files, databases, localStorage, IndexedDB, etc.) while LWK controls what is stored.
//!
//! For use with trait objects (`dyn`), see [`DynStore`] which provides an object-safe
//! version with boxed errors.

use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tempfile::NamedTempFile;

use crate::encrypt::{
    cipher_from_key_bytes, decrypt_with_nonce_prefix, encrypt_with_random_nonce, EncryptError,
};

/// A boxed error type for use with [`DynStore`].
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A generic key-value storage interface.
///
/// This trait uses `&self` for all methods, allowing implementations to use
/// interior mutability (e.g., `Mutex`) for thread-safe access.
///
/// Keys are `AsRef<[u8]>` for flexibility - both `&str` and `&[u8]` work.
/// Values are always `Vec<u8>` for binary serialization flexibility.
///
/// See [`MemoryStore`] for a simple in-memory implementation.
///
/// For use with trait objects, see [`DynStore`].
pub trait Store: Send + Sync + Debug {
    /// The error type returned by storage operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Retrieve a value by key.
    ///
    /// Returns `Ok(None)` if the key does not exist.
    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Insert or update a value.
    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error>;

    /// Remove a value by key.
    ///
    /// Returns `Ok(())` even if the key did not exist.
    fn remove<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error>; // TODO: Should return Option<Vec<u8>> of the removed value
}

/// An object-safe key-value storage trait for use with `dyn`.
///
/// This trait is similar to [`Store`] but uses concrete types instead of generics,
/// making it usable as a trait object (`dyn DynStore`).
///
/// The error type is boxed to allow different implementations to return different errors.
///
/// Any type implementing [`Store`] automatically implements `DynStore`.
pub trait DynStore: Send + Sync + Debug {
    /// Retrieve a value by key.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, BoxError>;
    /// Insert or update a value.
    fn put(&self, key: &str, value: &[u8]) -> Result<(), BoxError>;
    /// Remove a value by key.
    fn remove(&self, key: &str) -> Result<(), BoxError>;
}

/// Blanket implementation of [`DynStore`] for any type implementing [`Store`].
impl<S: Store> DynStore for S {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, BoxError> {
        Store::get(self, key).map_err(|e| Box::new(e) as BoxError)
    }

    fn put(&self, key: &str, value: &[u8]) -> Result<(), BoxError> {
        Store::put(self, key, value).map_err(|e| Box::new(e) as BoxError)
    }

    fn remove(&self, key: &str) -> Result<(), BoxError> {
        Store::remove(self, key).map_err(|e| Box::new(e) as BoxError)
    }
}

/// A simple in-memory implementation of [`Store`].
///
/// Useful for testing or ephemeral storage scenarios.
#[derive(Debug, Default)]
pub struct MemoryStore {
    data: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
}

impl MemoryStore {
    /// Create a new empty `MemoryStore`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    type Error = std::convert::Infallible;

    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self
            .data
            .lock()
            .expect("lock poisoned")
            .get(key.as_ref())
            .cloned())
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
        self.data
            .lock()
            .expect("lock poisoned")
            .insert(key.as_ref().to_vec(), value.as_ref().to_vec());
        Ok(())
    }

    fn remove<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error> {
        self.data
            .lock()
            .expect("lock poisoned")
            .remove(key.as_ref());
        Ok(())
    }
}

/// A [`Store`] implementation that intentionally persists nothing.
///
/// All reads return `None` and writes/removals are acknowledged but discarded.
#[derive(Debug, Default, Clone, Copy)]
pub struct FakeStore;

impl FakeStore {
    /// Create a new `FakeStore`.
    pub fn new() -> Self {
        Self
    }
}

impl Store for FakeStore {
    type Error = std::convert::Infallible;

    fn get<K: AsRef<[u8]>>(&self, _key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(None)
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, _key: K, _value: V) -> Result<(), Self::Error> {
        Ok(())
    }

    fn remove<K: AsRef<[u8]>>(&self, _key: K) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// A filesystem-backed implementation of [`Store`].
///
/// Each key/value is stored as a file under a root directory, where the filename
/// is derived from the key bytes using a deterministic, filesystem-safe encoding.
#[derive(Debug)]
pub struct FileStore {
    /// Root directory.
    ///
    /// Conceptually immutable: we only ever read/clone it. It is wrapped in a `Mutex`
    /// so that all store operations necessarily take the same lock, preventing
    /// interleaving `get`/`put`/`remove` across threads.
    root: Mutex<PathBuf>,
}
impl FileStore {
    /// Create a new `FileStore` rooted at `path`.
    ///
    /// The directory is created if missing. Returns an error if `path` exists and is a file.
    pub fn new(path: PathBuf) -> Result<Self, std::io::Error> {
        if path.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "FileStore root path is a file",
            ));
        }
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
        Ok(Self {
            root: Mutex::new(path),
        })
    }

    fn file_path(root: &Path, key: &[u8]) -> Result<PathBuf, std::io::Error> {
        // This store is intended as a drop-in replacement for `FsPersister`, which
        // uses UTF-8 file names like "000000000000". So we accept only UTF-8 keys
        // and map them 1:1 to filenames.
        let name = std::str::from_utf8(key).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "store key is not valid UTF-8",
            )
        })?;

        if name.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "store key is empty",
            ));
        }

        if name.len() > 255 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "store key exceeds maximum file name length (255 bytes)",
            ));
        }

        // Basic safety: forbid path separators and traversal components.
        if name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
            || name.contains('\0')
            || name.contains(':')
            || name.contains('*')
            || name.contains('?')
            || name.contains('<')
            || name.contains('>')
            || name.contains('|')
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "store key contains invalid file name characters",
            ));
        }

        Ok(root.join(name))
    }
}
impl Store for FileStore {
    type Error = std::io::Error;
    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let root = self.root.lock().expect("lock poisoned");
        let path = Self::file_path(&root, key.as_ref())?;
        match fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
        let root = self.root.lock().expect("lock poisoned");
        let path = Self::file_path(&root, key.as_ref())?;

        // Write to a temp file in the same directory then persist it atomically.
        let mut tmp = NamedTempFile::new_in(&*root)?;
        tmp.write_all(value.as_ref())?;
        tmp.as_file().sync_all()?;

        match tmp.persist(&path) {
            Ok(_) => {}
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                // Some platforms do not allow replacing an existing file via rename.
                // Remove the destination and retry with the same temp file.
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(remove_err) if remove_err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(remove_err) => return Err(remove_err),
                }

                e.file
                    .persist(&path)
                    .map_err(|persist_err| persist_err.error)?;
            }
            Err(e) => return Err(e.error),
        }

        Ok(())
    }

    fn remove<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error> {
        let root = self.root.lock().expect("lock poisoned");
        let path = Self::file_path(&root, key.as_ref())?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// Error type for [`EncryptedStore`] operations.
///
/// This wraps errors from the inner store as well as encryption/decryption errors.
#[derive(Debug)]
pub enum EncryptedStoreError<E: std::error::Error + Send + Sync + 'static> {
    /// Error from the inner store.
    Store(E),
    /// Error during encryption or decryption.
    Encrypt(EncryptError),
}

impl<E: std::error::Error + Send + Sync + 'static> std::fmt::Display for EncryptedStoreError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncryptedStoreError::Store(e) => write!(f, "store error: {e}"),
            EncryptedStoreError::Encrypt(e) => write!(f, "encryption error: {e}"),
        }
    }
}

impl<E: std::error::Error + Send + Sync + 'static> std::error::Error for EncryptedStoreError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EncryptedStoreError::Store(e) => Some(e),
            EncryptedStoreError::Encrypt(e) => Some(e),
        }
    }
}

/// A [`Store`] wrapper that encrypts values using AES-256-GCM-SIV.
///
/// All values are encrypted before being stored and decrypted when retrieved.
/// Keys are not encrypted and are passed through to the inner store unchanged.
///
/// This wrapper can be used with any [`Store`] implementation, for example wrapping
/// a [`FileStore`] to create encrypted persistent storage.
#[derive(Debug)]
pub struct EncryptedStore<S> {
    inner: S,
    key_bytes: [u8; 32],
}

impl<S> EncryptedStore<S> {
    /// Create a new `EncryptedStore` wrapping the given store with the provided key.
    ///
    /// The `key_bytes` should be a 32-byte encryption key. It is typically derived
    /// from a wallet descriptor or other secret material.
    pub fn new(inner: S, key_bytes: [u8; 32]) -> Self {
        Self { inner, key_bytes }
    }

    /// Get a reference to the inner store.
    pub fn inner(&self) -> &S {
        &self.inner
    }

    /// Consume this wrapper and return the inner store.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S: Store> Store for EncryptedStore<S> {
    type Error = EncryptedStoreError<S::Error>;

    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        match self.inner.get(key).map_err(EncryptedStoreError::Store)? {
            Some(ciphertext) => {
                let mut cipher = cipher_from_key_bytes(self.key_bytes);
                let plaintext = decrypt_with_nonce_prefix(&mut cipher, &ciphertext)
                    .map_err(EncryptedStoreError::Encrypt)?;
                Ok(Some(plaintext))
            }
            None => Ok(None),
        }
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
        let mut cipher = cipher_from_key_bytes(self.key_bytes);
        let ciphertext = encrypt_with_random_nonce(&mut cipher, value.as_ref())
            .map_err(EncryptedStoreError::Encrypt)?;
        self.inner
            .put(key, ciphertext)
            .map_err(EncryptedStoreError::Store)?;
        Ok(())
    }

    fn remove<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error> {
        self.inner.remove(key).map_err(EncryptedStoreError::Store)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{EncryptedStore, FakeStore, FileStore, MemoryStore, Store};

    #[test]
    fn memory_store() {
        let store = MemoryStore::new();

        // Get non-existent key returns None
        assert_eq!(store.get("key").unwrap(), None);

        // Put and get
        store.put("key", b"value").unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"value".to_vec()));

        // Overwrite
        store.put("key", b"new_value").unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"new_value".to_vec()));

        // Remove
        store.remove("key").unwrap();
        assert_eq!(store.get("key").unwrap(), None);

        // Remove non-existent key is ok
        store.remove("key").unwrap();
    }

    #[test]
    fn file_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore::new(dir.path().to_path_buf()).unwrap();

        // Get non-existent key returns None
        assert_eq!(store.get("key").unwrap(), None);

        // Put and get
        store.put("key", b"value").unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"value".to_vec()));

        store.put("key2", b"value2").unwrap();
        assert_eq!(store.get("key2").unwrap(), Some(b"value2".to_vec()));

        // Overwrite
        store.put("key", b"new_value").unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"new_value".to_vec()));

        // Non-UTF8 keys are rejected (this store maps keys directly to filenames).
        let non_utf8_key = [0u8, 255u8, 1u8];
        assert!(store.put(non_utf8_key, b"bin").is_err());

        // Remove
        store.remove("key").unwrap();
        assert_eq!(store.get("key").unwrap(), None);

        // Remove non-existent key is ok
        store.remove("key").unwrap();

        drop(store);
        // Check that the file is still there
        let store = FileStore::new(dir.path().to_path_buf()).unwrap();

        assert_eq!(store.get("key").unwrap(), None);
        assert_eq!(store.get("key2").unwrap(), Some(b"value2".to_vec()));
    }

    #[test]
    fn fake_store() {
        let store = FakeStore::new();

        assert_eq!(store.get("key").unwrap(), None);
        store.put("key", b"value").unwrap();
        assert_eq!(store.get("key").unwrap(), None);
        store.remove("key").unwrap();
    }

    #[test]
    fn encrypted_store_memory() {
        let key_bytes = [7u8; 32];
        let inner = MemoryStore::new();
        let store = EncryptedStore::new(inner, key_bytes);

        // Get non-existent key returns None
        assert_eq!(store.get("key").unwrap(), None);

        // Put and get - value should be decrypted transparently
        store.put("key", b"secret value").unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"secret value".to_vec()));

        // Verify data is actually encrypted in inner store
        let raw = store.inner().get("key").unwrap().unwrap();
        assert_ne!(raw, b"secret value".to_vec());

        // Overwrite
        store.put("key", b"new secret").unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"new secret".to_vec()));

        // Remove
        store.remove("key").unwrap();
        assert_eq!(store.get("key").unwrap(), None);
    }

    #[test]
    fn encrypted_store_file() {
        let key_bytes = [42u8; 32];
        let dir = tempfile::tempdir().unwrap();
        let inner = FileStore::new(dir.path().to_path_buf()).unwrap();
        let store = EncryptedStore::new(inner, key_bytes);

        // Put and get
        store.put("000000000000", b"update data").unwrap();
        assert_eq!(
            store.get("000000000000").unwrap(),
            Some(b"update data".to_vec())
        );

        // Verify the file contains encrypted (not plaintext) data
        let file_path = dir.path().join("000000000000");
        let raw_bytes = std::fs::read(&file_path).unwrap();
        assert_ne!(raw_bytes, b"update data".to_vec());

        // Persistence: drop and recreate
        drop(store);
        let inner = FileStore::new(dir.path().to_path_buf()).unwrap();
        let store = EncryptedStore::new(inner, key_bytes);
        assert_eq!(
            store.get("000000000000").unwrap(),
            Some(b"update data".to_vec())
        );

        // Wrong key cannot decrypt
        let inner = FileStore::new(dir.path().to_path_buf()).unwrap();
        let wrong_store = EncryptedStore::new(inner, [0u8; 32]);
        assert!(wrong_store.get("000000000000").is_err());
    }
}
