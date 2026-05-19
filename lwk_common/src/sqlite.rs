//! SQLite-backed implementation of [`Store`].

use std::path::Path;
use std::sync::Mutex;

use rusqlite::params;

use crate::store::Store;

#[allow(missing_docs)]
#[derive(thiserror::Error, Debug)]
pub enum SqliteStoreError {
    #[error(transparent)]
    Rusqlite(#[from] rusqlite::Error),
}

/// A SQLite-backed implementation of [`Store`]
#[derive(Debug)]
pub struct SqliteStore {
    conn: Mutex<rusqlite::Connection>,
}

impl SqliteStore {
    /// Open or create a `SqliteStore` at `path`.
    ///
    /// The store table is created if it does not already exist.
    pub fn new(path: &Path) -> Result<Self, SqliteStoreError> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS store (
                 key   BLOB PRIMARY KEY NOT NULL,
                 value BLOB NOT NULL
             );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl Store for SqliteStore {
    type Error = SqliteStoreError;

    fn is_persisted(&self) -> bool {
        true
    }

    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let conn = self.conn.lock().expect("lock poisoned");
        let mut stmt = conn.prepare_cached("SELECT value FROM store WHERE key = ?1")?;
        let mut rows = stmt.query(params![key.as_ref()])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
        let conn = self.conn.lock().expect("lock poisoned");
        let mut stmt =
            conn.prepare_cached("INSERT OR REPLACE INTO store (key, value) VALUES (?1, ?2)")?;
        stmt.execute(params![key.as_ref(), value.as_ref()])?;
        Ok(())
    }

    fn remove<K: AsRef<[u8]>>(&self, key: K) -> Result<(), Self::Error> {
        let conn = self.conn.lock().expect("lock poisoned");
        let mut stmt = conn.prepare_cached("DELETE FROM store WHERE key = ?1")?;
        stmt.execute(params![key.as_ref()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_store() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let store = SqliteStore::new(&db_path).unwrap();

        assert!(store.is_persisted());

        // missing key returns None
        assert_eq!(store.get("key").unwrap(), None);

        // put and get
        store.put("key", b"value").unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"value".to_vec()));

        // overwrite
        store.put("key", b"new_value").unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"new_value".to_vec()));

        // remove
        store.put("key2", b"value2").unwrap();
        store.remove("key2").unwrap();
        assert_eq!(store.get("key2").unwrap(), None);

        // remove missing key is ok
        store.remove("key2").unwrap();

        // binary keys are allowed (unlike FileStore)
        let bin_key = [0u8, 255u8, 1u8];
        store.put(bin_key, b"bin_value").unwrap();
        assert_eq!(store.get(bin_key).unwrap(), Some(b"bin_value".to_vec()));

        // persistence: drop and reopen
        drop(store);
        let store = SqliteStore::new(&db_path).unwrap();
        assert_eq!(store.get("key").unwrap(), Some(b"new_value".to_vec()));
        assert_eq!(store.get(bin_key).unwrap(), Some(b"bin_value".to_vec()));
    }
}
