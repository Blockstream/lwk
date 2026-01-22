use std::sync::Arc;

use crate::LwkError;

/// An FFI-safe key-value storage trait for caller-defined persistence.
///
/// Keys are strings to allow namespacing (e.g., "Liquid:Tx:abcd1234").
/// Values are byte arrays for flexibility in serialization.
#[uniffi::export(with_foreign)]
pub trait ForeignStore: Send + Sync {
    /// Retrieve a value by key.
    ///
    /// Returns `Ok(None)` if the key does not exist.
    fn get(&self, key: String) -> Result<Option<Vec<u8>>, LwkError>;

    /// Insert or update a value.
    fn put(&self, key: String, value: Vec<u8>) -> Result<(), LwkError>;

    /// Remove a value by key.
    ///
    /// Returns `Ok(())` even if the key did not exist.
    fn delete(&self, key: String) -> Result<(), LwkError>;
}

/// Error type for the store bridge.
#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    /// Error from the foreign store
    #[error("{0}")]
    Foreign(String),
}

/// A bridge that connects a [`ForeignStore`] to [`lwk_common::Store`].
#[derive(uniffi::Object, Debug)]
pub struct ForeignStoreLink {
    inner: Arc<dyn ForeignStore>,
}

// Manual Debug impl not possible for dyn trait, so we need a wrapper
impl std::fmt::Debug for dyn ForeignStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ForeignStore")
    }
}

#[uniffi::export]
impl ForeignStoreLink {
    /// Create a new `ForeignStoreLink` from a foreign store implementation.
    #[uniffi::constructor]
    pub fn new(store: Arc<dyn ForeignStore>) -> Self {
        Self { inner: store }
    }
}

impl lwk_common::Store for ForeignStoreLink {
    type Error = StoreError;

    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let key_str = String::from_utf8_lossy(key.as_ref()).into_owned();
        self.inner
            .get(key_str)
            .map_err(|e| StoreError::Foreign(format!("{e:?}")))
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
        let key_str = String::from_utf8_lossy(key.as_ref()).into_owned();
        self.inner
            .put(key_str, value.as_ref().to_vec())
            .map_err(|e| StoreError::Foreign(format!("{e:?}")))
    }

    fn delete<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error> {
        let key_str = String::from_utf8_lossy(key.as_ref()).into_owned();
        self.inner
            .delete(key_str)
            .map_err(|e| StoreError::Foreign(format!("{e:?}")))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use lwk_common::Store;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct TestStore {
        data: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl TestStore {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                data: Mutex::new(HashMap::new()),
            })
        }
    }

    impl ForeignStore for TestStore {
        fn get(&self, key: String) -> Result<Option<Vec<u8>>, LwkError> {
            Ok(self.data.lock().unwrap().get(&key).cloned())
        }

        fn put(&self, key: String, value: Vec<u8>) -> Result<(), LwkError> {
            self.data.lock().unwrap().insert(key, value);
            Ok(())
        }

        fn delete(&self, key: String) -> Result<(), LwkError> {
            self.data.lock().unwrap().remove(&key);
            Ok(())
        }
    }

    #[test]
    fn foreign_store_link() {
        let foreign = TestStore::new();
        let link = ForeignStoreLink::new(foreign);

        // Test through the lwk_common::Store interface
        assert_eq!(link.get("key").unwrap(), None);

        link.put("key", b"value").unwrap();
        assert_eq!(link.get("key").unwrap(), Some(b"value".to_vec()));

        link.delete("key").unwrap();
        assert_eq!(link.get("key").unwrap(), None);
    }
}
