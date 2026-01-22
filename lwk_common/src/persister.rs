//! Generic persistence interface for key-value storage.
//!
//! This module defines the [`Persister`] trait, which provides a simple key-value
//! storage abstraction. Implementations can back this with various storage backends
//! (files, databases, localStorage, IndexedDB, etc.) while LWK controls what is stored.

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Mutex;

/// A generic key-value persistence interface.
///
/// This trait uses `&self` for all methods, allowing implementations to use
/// interior mutability (e.g., `Mutex`) for thread-safe access.
///
/// Keys are `AsRef<[u8]>` for flexibility - both `&str` and `&[u8]` work.
/// Values are always `Vec<u8>` for binary serialization flexibility.
///
/// See [`MemoryPersister`] for a simple in-memory implementation.
pub trait Persister: Send + Sync + Debug {
    /// The error type returned by persistence operations.
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
    fn delete<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error>;
}

/// A simple in-memory implementation of [`Persister`].
///
/// Useful for testing or ephemeral storage scenarios.
#[derive(Debug, Default)]
pub struct MemoryPersister {
    data: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
}

impl MemoryPersister {
    /// Create a new empty `MemoryPersister`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Persister for MemoryPersister {
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

    fn delete<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error> {
        self.data
            .lock()
            .expect("lock poisoned")
            .remove(key.as_ref());
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn memory_persister() {
        let persister = MemoryPersister::new();

        // Get non-existent key returns None
        assert_eq!(persister.get("key").unwrap(), None);

        // Put and get
        persister.put("key", b"value").unwrap();
        assert_eq!(persister.get("key").unwrap(), Some(b"value".to_vec()));

        // Overwrite
        persister.put("key", b"new_value").unwrap();
        assert_eq!(persister.get("key").unwrap(), Some(b"new_value".to_vec()));

        // Delete
        persister.delete("key").unwrap();
        assert_eq!(persister.get("key").unwrap(), None);

        // Delete non-existent key is ok
        persister.delete("key").unwrap();
    }
}
