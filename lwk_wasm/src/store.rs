//! Duck-typed JavaScript storage interface for WASM.
//!
//! JavaScript doesn't have traits, so we use duck typing: any JS object with
//! `get`, `put`, and `delete` methods can be used as storage.

use wasm_bindgen::prelude::*;

use crate::Error;

// Duck-typed JavaScript storage interface.
// The JS object must have these methods - Rust will call them directly.
#[wasm_bindgen]
extern "C" {
    /// A duck-typed JavaScript storage object.
    ///
    /// Any JS object with `get(key) -> Uint8Array|null`, `put(key, value)`,
    /// and `delete(key)` methods can be used.
    ///
    /// Example JS implementation:
    /// ```js
    /// const storage = {
    ///     _data: new Map(),
    ///     get(key) { return this._data.get(key) || null; },
    ///     put(key, value) { this._data.set(key, value); },
    ///     delete(key) { this._data.delete(key); }
    /// };
    /// ```
    pub type JsStorage;

    /// Retrieve a value by key. Returns null if not found.
    #[wasm_bindgen(method, catch)]
    fn get(this: &JsStorage, key: &str) -> Result<Option<Vec<u8>>, JsValue>;

    /// Store a key-value pair.
    #[wasm_bindgen(method, catch)]
    fn put(this: &JsStorage, key: &str, value: &[u8]) -> Result<(), JsValue>;

    /// Delete a key.
    #[wasm_bindgen(method, catch)]
    fn delete(this: &JsStorage, key: &str) -> Result<(), JsValue>;
}

/// Error type for the JS store bridge.
#[derive(thiserror::Error, Debug)]
pub enum JsStoreError {
    /// Error from JavaScript
    #[error("{0}")]
    Js(String),
}

/// A bridge that connects a [`JsStorage`] to [`lwk_common::Store`].
#[wasm_bindgen]
pub struct JsStoreLink {
    inner: JsStorage,
}

impl std::fmt::Debug for JsStoreLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsStoreLink")
    }
}

#[wasm_bindgen]
impl JsStoreLink {
    /// Create a new `JsStoreLink` from a JavaScript storage object.
    ///
    /// The JS object must have `get(key)`, `put(key, value)`, and `delete(key)` methods.
    #[wasm_bindgen(constructor)]
    pub fn new(storage: JsStorage) -> Self {
        Self { inner: storage }
    }
}

impl lwk_common::Store for JsStoreLink {
    type Error = JsStoreError;

    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let key_str = String::from_utf8_lossy(key.as_ref());
        self.inner
            .get(&key_str)
            .map_err(|e| JsStoreError::Js(format!("{e:?}")))
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
        let key_str = String::from_utf8_lossy(key.as_ref());
        self.inner
            .put(&key_str, value.as_ref())
            .map_err(|e| JsStoreError::Js(format!("{e:?}")))
    }

    fn delete<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error> {
        let key_str = String::from_utf8_lossy(key.as_ref());
        self.inner
            .delete(&key_str)
            .map_err(|e| JsStoreError::Js(format!("{e:?}")))
    }
}

// Send + Sync are required by lwk_common::Store
// Safety: WASM is single-threaded, so these are safe to implement
unsafe impl Send for JsStoreLink {}
unsafe impl Sync for JsStoreLink {}

/// Test helper to verify Rust can read/write through a JS store.
#[wasm_bindgen]
pub struct JsTestStore {
    store: JsStoreLink,
}

#[wasm_bindgen]
impl JsTestStore {
    /// Create a new test helper wrapping the given JS storage.
    #[wasm_bindgen(constructor)]
    pub fn new(storage: JsStorage) -> Self {
        Self {
            store: JsStoreLink::new(storage),
        }
    }

    /// Write a key-value pair to the store.
    pub fn write(&self, key: &str, value: &[u8]) -> Result<(), Error> {
        use lwk_common::Store;
        self.store
            .put(key, value)
            .map_err(|e| Error::Generic(format!("{e}")))
    }

    /// Read a value from the store.
    pub fn read(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        use lwk_common::Store;
        self.store
            .get(key)
            .map_err(|e| Error::Generic(format!("{e}")))
    }

    /// Delete a key from the store.
    pub fn delete(&self, key: &str) -> Result<(), Error> {
        use lwk_common::Store;
        self.store
            .delete(key)
            .map_err(|e| Error::Generic(format!("{e}")))
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use lwk_common::Store;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    // Inline JS to create a test storage object
    #[wasm_bindgen(inline_js = r#"
        export function createTestStorage() {
            const store = new Map();
            return {
                get: function(key) {
                    return store.get(key) || null;
                },
                put: function(key, value) {
                    store.set(key, value);
                },
                delete: function(key) {
                    store.delete(key);
                }
            };
        }
    "#)]
    extern "C" {
        #[wasm_bindgen(js_name = createTestStorage)]
        fn create_test_storage() -> JsStorage;
    }

    #[wasm_bindgen_test]
    fn test_js_store_link() {
        let storage = create_test_storage();
        let link = JsStoreLink::new(storage);

        // Test get non-existent key
        assert_eq!(link.get("key").unwrap(), None);

        // Test put and get
        link.put("key", b"value").unwrap();
        assert_eq!(link.get("key").unwrap(), Some(b"value".to_vec()));

        // Test overwrite
        link.put("key", b"new_value").unwrap();
        assert_eq!(link.get("key").unwrap(), Some(b"new_value".to_vec()));

        // Test delete
        link.delete("key").unwrap();
        assert_eq!(link.get("key").unwrap(), None);

        // Test delete non-existent key
        link.delete("key").unwrap();

        // Test with namespaced keys
        link.put("Liquid:Tx:abc123", b"tx_data").unwrap();
        assert_eq!(
            link.get("Liquid:Tx:abc123").unwrap(),
            Some(b"tx_data".to_vec())
        );
    }

    #[wasm_bindgen_test]
    fn test_js_test_store() {
        let storage = create_test_storage();
        let test = JsTestStore::new(storage);

        // Test Rust writing and reading through JsTestStore
        test.write("key", b"value").unwrap();
        assert_eq!(test.read("key").unwrap(), Some(b"value".to_vec()));

        test.delete("key").unwrap();
        assert_eq!(test.read("key").unwrap(), None);
    }
}
