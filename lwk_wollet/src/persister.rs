use std::{
    fmt::Display,
    fs,
    ops::Add,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

use elements::{bitcoin::hashes::Hash, hashes::sha256t_hash_newtype};

use crate::{ElementsNetwork, Error, Update, WolletDescriptor};

#[derive(thiserror::Error, Debug)]
pub enum PersistError {
    #[error(transparent)]
    Encoding(#[from] elements::encode::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub trait Persister {
    /// Return ith elements inserted
    fn get(&self, index: usize) -> Result<Option<Update>, PersistError>;

    /// Push and persist an update.
    ///
    /// Implementors are encouraged to coalesce consequent updates with `update.only_tip() == true`
    fn push(&self, update: Update) -> Result<(), PersistError>;
}

sha256t_hash_newtype! {
    /// The tag of the hash
    pub struct DirectoryIdTag = hash_str("LWK-FS-Directory-Id/1.0");

    /// A tagged hash to generate the name of the subdirectory to store cache content
    #[hash_newtype(forward)]
    pub struct DirectoryIdHash(_);
}

/// Implementation of a [`Persister`] which persist nothing.
pub struct NoPersist {}

impl NoPersist {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {})
    }
}

impl Persister for NoPersist {
    fn get(&self, _index: usize) -> Result<Option<Update>, PersistError> {
        Ok(None)
    }

    fn push(&self, _update: Update) -> Result<(), PersistError> {
        Ok(())
    }
}

struct FsPersisterInner {
    /// Directory where the data files will be written
    path: PathBuf,

    /// Next free position to write an update
    next: Counter,

    /// used to create the cipher to encrypt data
    desc: WolletDescriptor,
}

/// A file system persister that writes encrypted incremental updates
pub struct FsPersister {
    inner: Mutex<FsPersisterInner>,
}

impl FsPersister {
    /// Creates a persister of updates. While being written they are encrypted using a key derived
    /// from the given descriptor.
    /// From the given path create a network subdirectory with
    /// another subdirectory which name is one-way derived from the descriptor
    pub fn new<P: AsRef<Path>>(
        path: P,
        network: ElementsNetwork,
        desc: &WolletDescriptor,
    ) -> Result<Arc<Self>, Error> {
        let mut path = path.as_ref().to_path_buf();
        path.push(network.as_str());
        path.push("enc_cache");
        path.push(DirectoryIdHash::hash(desc.to_string().as_bytes()).to_string());
        if path.is_file() {
            return Err(Error::Generic("given path is a file".to_string()));
        }
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
        let mut next = Counter::default();
        for el in path.read_dir()? {
            let entry = &el?;
            if entry.path().is_file() {
                let file_name = entry.file_name();
                let name = file_name.to_str();
                if let Some(name) = name {
                    let counter: Counter = name.parse()?;
                    next = next.max(counter + 1);
                }
            }
        }

        Ok(Arc::new(Self {
            inner: Mutex::new(FsPersisterInner {
                path,
                next,
                desc: desc.clone(),
            }),
        }))
    }
}

impl FsPersisterInner {
    fn path(&self, counter: &Counter) -> PathBuf {
        let mut path = self.path.clone();
        path.push(counter.to_string());
        path
    }

    fn last(&self) -> Result<Option<Update>, PersistError> {
        if self.next.0 == 0 {
            return Ok(None);
        }
        self.get(self.next.0 - 1)
    }

    fn get(&self, index: usize) -> Result<Option<Update>, PersistError> {
        let next = self.next.0;
        if index < next {
            let path = self.path(&Counter::from(index));
            let bytes = fs::read(path)?;

            Ok(Some(
                Update::deserialize_decrypted(&bytes, &self.desc)
                    .map_err(|e| PersistError::Other(e.to_string()))?,
            ))
        } else {
            Ok(None)
        }
    }
}

fn to_other<D: std::fmt::Debug>(d: D) -> PersistError {
    PersistError::Other(format!("{d:?}"))
}

impl Persister for FsPersister {
    fn get(&self, index: usize) -> Result<Option<Update>, PersistError> {
        let inner = self.inner.lock().map_err(to_other)?;
        inner.get(index)
    }

    fn push(&self, mut update: Update) -> Result<(), PersistError> {
        let mut inner = self.inner.lock().map_err(to_other)?;
        if update.only_tip() {
            if let Ok(Some(prev_update)) = inner.last() {
                if prev_update.only_tip() {
                    // since this update and the last are only an update of the tip, we can
                    // overwrite last update instead of creating a new file.
                    // But we need to update the wallet status so that there will be no problem in reapplying it
                    update.wollet_status = prev_update.wollet_status;
                    inner.next = (inner.next.0 - 1).into() // safety: next is at least 1 or last() would be None
                }
            }
        }
        let path = inner.path(&inner.next);
        let ciphertext = update
            .serialize_encrypted(&inner.desc)
            .map_err(|e| PersistError::Other(e.to_string()))?;

        fs::write(path, ciphertext)?;
        inner.next = inner.next.clone() + 1;
        Ok(())
    }
}

const PERSISTED_FILE_NAME_LENGTH: usize = 12;

/// Encapsulate an usize so that its to/from string representation are coherent
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default, Clone)]
struct Counter(usize);

impl Display for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:0>width$}", self.0, width = PERSISTED_FILE_NAME_LENGTH)
    }
}
impl FromStr for Counter {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != PERSISTED_FILE_NAME_LENGTH {
            return Err(Error::Generic("Not 12 chars".to_string()));
        }
        let c: usize = s.parse()?;
        Ok(Self(c))
    }
}
impl From<usize> for Counter {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
impl From<&Counter> for usize {
    fn from(value: &Counter) -> Self {
        value.0
    }
}
impl From<Counter> for usize {
    fn from(value: Counter) -> Self {
        value.0
    }
}
impl Add<usize> for Counter {
    type Output = Counter;

    fn add(self, rhs: usize) -> Self::Output {
        Counter(rhs + self.0)
    }
}

#[cfg(test)]
mod test {
    use std::{
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use crate::{ElementsNetwork, FsPersister, PersistError, Update, WolletDescriptor};

    use super::{Counter, NoPersist, Persister};

    struct MemoryPersister(Mutex<Vec<Update>>);
    impl MemoryPersister {
        pub fn new() -> Arc<Self> {
            Arc::new(Self(Mutex::new(vec![])))
        }
    }
    impl Persister for MemoryPersister {
        fn get(&self, index: usize) -> Result<Option<Update>, PersistError> {
            Ok(self.0.lock().unwrap().get(index).cloned())
        }

        fn push(&self, update: crate::Update) -> Result<(), PersistError> {
            self.0.lock().unwrap().push(update);
            Ok(())
        }
    }

    fn inner_test_persister(persister: Arc<dyn Persister>, first_time: bool) {
        if first_time {
            assert_eq!(persister.get(0).unwrap(), None);
        }

        let update1 = Update::deserialize(&lwk_test_util::update_test_vector_bytes()).unwrap();
        let update2 = {
            let mut update2 = update1.clone();
            update2.timestamps.push((22, 55));
            update2
        };
        assert_ne!(&update1, &update2);

        if first_time {
            persister.push(update1.clone()).unwrap();
            assert_eq!(persister.get(0).unwrap().unwrap(), update1.clone());
            assert!(persister.get(1).unwrap().is_none());

            persister.push(update2.clone()).unwrap();
        }
        assert_eq!(persister.get(0).unwrap().unwrap(), update1);
        assert_eq!(persister.get(1).unwrap().unwrap(), update2);
        assert!(persister.get(2).unwrap().is_none());
    }

    fn wollet_descriptor_test_vector() -> WolletDescriptor {
        let exp = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d";
        WolletDescriptor::from_str(exp).unwrap()
    }

    #[test]
    fn test_memory_persister() {
        let persister = MemoryPersister::new();
        inner_test_persister(persister, true);
    }

    #[test]
    fn test_no_persist() {
        let persister = NoPersist {};
        assert_eq!(persister.get(0).unwrap(), None);
        let update = Update::deserialize(&lwk_test_util::update_test_vector_bytes()).unwrap();
        persister.push(update).unwrap();
        assert_eq!(persister.get(0).unwrap(), None);
    }

    #[test]
    fn test_encrypted_fs_persister() {
        let tempdir = tempfile::tempdir().unwrap();
        let desc = wollet_descriptor_test_vector();
        let n = ElementsNetwork::LiquidTestnet;
        let persister = FsPersister::new(&tempdir, n, &desc).unwrap();
        inner_test_persister(persister, true);
        let persister = FsPersister::new(&tempdir, n, &desc).unwrap();
        inner_test_persister(persister, false);
    }

    #[test]
    fn test_counter() {
        let c = Counter::default();
        assert_eq!(c.to_string(), "000000000000");
        assert_eq!(usize::from(c), 0);
        assert_eq!(Counter::from(100).to_string(), "000000000100");
    }
}
