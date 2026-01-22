//! Generic persistence interface for key-value storage.
//!
//! This module defines the [`Persister`] trait, which provides a simple key-value
//! storage abstraction. Implementations can back this with various storage backends
//! (files, databases, localStorage, IndexedDB, etc.) while LWK controls what is stored.

use std::fmt::Debug;

/// A generic key-value persistence interface.
///
/// This trait uses `&self` for all methods, allowing implementations to use
/// interior mutability (e.g., `Mutex`, `RwLock`) for thread-safe access.
///
/// Keys are `AsRef<[u8]>` for flexibility - both `&str` and `&[u8]` work.
/// Values are always `Vec<u8>` for binary serialization flexibility.
///
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
