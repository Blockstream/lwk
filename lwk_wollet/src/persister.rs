use std::{
    fmt::Display,
    fs::{self},
    ops::Add,
    path::{Path, PathBuf},
    str::FromStr,
};

use aes_gcm_siv::{
    aead::{generic_array::GenericArray, AeadInPlace, NewAead},
    Aes256GcmSiv,
};
use elements::{bitcoin::hashes::Hash, hashes::sha256t_hash_newtype};
use rand::{thread_rng, Rng};

use crate::{ElementsNetwork, Error, Update, WolletDescriptor};

pub trait Persister {
    /// Return ith elements inserted
    fn get(&self, index: usize) -> Result<Option<Update>, Error>;

    /// Push and persist an update. Returns the number of updates persisted
    ///
    /// Implementors are encouraged to coalesce consequent updates with `update.only_tip() == true`
    fn push(&mut self, update: Update) -> Result<usize, Error>;
}

sha256t_hash_newtype! {
    /// The tag of the hash
    pub struct EncryptionKeyTag = hash_str("LWK-FS-Encryption-Key/1.0");

    /// A tagged hash to generate the key for encryption in the encrypted file system persister
    #[hash_newtype(forward)]
    pub struct EncryptionKeyHash(_);
}

sha256t_hash_newtype! {
    /// The tag of the hash
    pub struct DirectoryIdTag = hash_str("LWK-FS-Directory-Id/1.0");

    /// A tagged hash to generate the name of the subdirectory to store cache content
    #[hash_newtype(forward)]
    pub struct DirectoryIdHash(_);
}

pub struct NoPersist {}

impl NoPersist {
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }
}

impl Persister for NoPersist {
    fn get(&self, _index: usize) -> Result<Option<Update>, Error> {
        Ok(None)
    }

    fn push(&mut self, _update: Update) -> Result<usize, Error> {
        Ok(0)
    }
}

/// A file system persister that writes encrypted incremental updates
pub struct FsPersister {
    /// Directory where the data files will be written
    path: PathBuf,

    /// Next free position to write an update
    next: Counter,

    /// Cipher used to encrypt data
    cipher: Aes256GcmSiv,
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
    ) -> Result<Box<Self>, Error> {
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
        let key_bytes = EncryptionKeyHash::hash(desc.to_string().as_bytes()).to_byte_array();
        let key = GenericArray::from_slice(&key_bytes);
        let cipher = Aes256GcmSiv::new(key);

        Ok(Box::new(Self { path, next, cipher }))
    }

    fn path(&self, counter: &Counter) -> PathBuf {
        let mut path = self.path.clone();
        path.push(counter.to_string());
        path
    }

    fn read(&self, current: usize) -> Result<Update, Error> {
        let path = self.path(&Counter::from(current));
        let bytes = fs::read(path)?;

        let nonce_bytes = &bytes[..12];
        let mut ciphertext = bytes[12..].to_vec();

        let nonce = GenericArray::from_slice(nonce_bytes);

        self.cipher.decrypt_in_place(nonce, b"", &mut ciphertext)?;
        let plaintext = ciphertext;

        Ok(Update::deserialize(&plaintext)?)
    }

    /// Write at next position without incrementing the next counter
    /// returns the number of bytes written
    fn write(&mut self, update: Update) -> Result<usize, Error> {
        let path = self.path(&self.next);
        let mut plaintext = update.serialize()?;

        let mut nonce_bytes = [0u8; 12];
        thread_rng().fill(&mut nonce_bytes);
        let nonce = GenericArray::from_slice(&nonce_bytes);

        self.cipher.encrypt_in_place(nonce, b"", &mut plaintext)?;
        let ciphertext = plaintext;

        let mut file_content = vec![];
        file_content.extend(nonce_bytes);
        file_content.extend(ciphertext);

        fs::write(path, &file_content)?;
        Ok(file_content.len())
    }
}

struct EncryptedFsPersisterIter<'a> {
    current: usize,
    persister: &'a FsPersister,
}
impl<'a> Iterator for EncryptedFsPersisterIter<'a> {
    type Item = Result<Update, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = usize::from(&self.persister.next);
        if self.current < next {
            let update = self.persister.read(self.current);
            match update {
                Ok(update) => {
                    self.current += 1;
                    Some(Ok(update))
                }
                Err(e) => Some(Err(e)),
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let l = usize::from(&self.persister.next);
        (l, Some(l))
    }
}
impl<'a> ExactSizeIterator for EncryptedFsPersisterIter<'a> {}

impl Persister for FsPersister {
    fn get(&self, index: usize) -> Result<Option<Update>, Error> {
        if index < self.next.0 {
            self.read(index).map(Some)
        } else {
            Ok(None)
        }
    }

    fn push(&mut self, update: Update) -> Result<usize, Error> {
        tracing::debug!("FsPersister is pushing data, next is {:?}", self.next);
        let _ = self.write(update)?;
        self.next = self.next.clone() + 1;
        Ok((&self.next).into())
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
    use std::str::FromStr;

    use crate::{ElementsNetwork, Error, FsPersister, Update, WolletDescriptor};

    use super::{Counter, NoPersist, Persister};

    struct MemoryPersister(Vec<Update>);
    impl MemoryPersister {
        pub fn new() -> Box<Self> {
            Box::new(Self(vec![]))
        }
    }
    impl Persister for MemoryPersister {
        fn get(&self, index: usize) -> Result<Option<Update>, Error> {
            Ok(self.0.get(index).cloned())
        }

        fn push(&mut self, update: crate::Update) -> Result<usize, Error> {
            self.0.push(update);
            Ok(self.0.len())
        }
    }

    fn inner_test_persister(mut persister: Box<dyn Persister>, first_time: bool) {
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
            let el = persister.push(update1.clone()).unwrap();
            assert_eq!(el, 1);
            assert_eq!(persister.get(0).unwrap().unwrap(), update1.clone());
            assert!(persister.get(1).unwrap().is_none());

            let el = persister.push(update2.clone()).unwrap();
            assert_eq!(el, 2);
        }
        assert_eq!(persister.get(0).unwrap().unwrap(), update1);
        assert_eq!(persister.get(1).unwrap().unwrap(), update2);
        assert!(persister.get(2).unwrap().is_none());
    }

    pub fn wollet_descriptor_test_vector() -> WolletDescriptor {
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
        let mut persister = NoPersist {};
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
