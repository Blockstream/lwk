/// A [`lwk_common::Store`] implementation that intentionally panics on reads.
///
/// This is useful in tests that need to assert a code path does not read from
/// storage. Writes/removals are acknowledged but discarded, like
/// [`lwk_common::FakeStore`].
#[derive(Debug, Default, Clone)]
pub struct PanicStore {
    persisted: bool,
    exceptions: Vec<String>,
}

impl PanicStore {
    /// Create a new `PanicStore`.
    pub fn new(persisted: bool, exceptions: Vec<String>) -> Self {
        Self {
            persisted,
            exceptions,
        }
    }
}

impl lwk_common::Store for PanicStore {
    type Error = std::convert::Infallible;

    fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Vec<u8>>, Self::Error> {
        let key = String::from_utf8_lossy(key.as_ref()).to_string();
        if self.exceptions.contains(&key) {
            Ok(None)
        } else {
            panic!("PanicStore::get called for {key}")
        }
    }

    fn put<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, _key: K, _value: V) -> Result<(), Self::Error> {
        Ok(())
    }

    fn remove<K: AsRef<[u8]>>(&self, _key: K) -> Result<(), Self::Error> {
        Ok(())
    }

    fn is_persisted(&self) -> bool {
        self.persisted
    }
}
