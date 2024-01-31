use std::{
    fmt::Display,
    fs::{self},
    ops::Add,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{Error, Update};

trait Persister {
    /// Return the elements in the same order as they have been inserted
    fn iter(&self) -> impl ExactSizeIterator<Item = Update>;

    /// Push and persist an update.
    ///
    /// Implementors could decide to override previous element if both previous element and current
    /// has `update.only_tip() == true`
    fn push(&mut self, update: Update);
}

pub struct FsPersister {
    /// Directory where the data files will be written
    path: PathBuf,

    /// Next free position to write an update
    next: Counter,
}
impl FsPersister {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
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

        Ok(Self { path, next })
    }

    fn path(&self, counter: &Counter) -> PathBuf {
        let mut path = self.path.clone();
        path.push(counter.to_string());
        path
    }

    fn read(&self, current: usize) -> Result<Update, Error> {
        let path = self.path(&Counter::from(current));
        let bytes = fs::read(path)?;
        Ok(Update::deserialize(&bytes)?)
    }

    /// Write at next position without incrementing the next counter
    /// returns the number of writes written
    fn write(&mut self, update: Update) -> Result<usize, Error> {
        let path = self.path(&self.next);
        let bytes = update.serialize()?;
        fs::write(path, &bytes)?;
        Ok(bytes.len())
    }
}

struct FsPersisterIter<'a> {
    current: usize,
    persister: &'a FsPersister,
}
impl<'a> Iterator for FsPersisterIter<'a> {
    type Item = Update;

    fn next(&mut self) -> Option<Self::Item> {
        let next = usize::from(&self.persister.next);
        if self.current < next {
            let update = self
                .persister
                .read(self.current)
                .expect("checker current<next");
            self.current += 1;
            Some(update)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let l = usize::from(&self.persister.next);
        (l, Some(l))
    }
}
impl<'a> ExactSizeIterator for FsPersisterIter<'a> {}

impl Persister for FsPersister {
    fn iter(&self) -> impl ExactSizeIterator<Item = Update> {
        FsPersisterIter {
            current: 0,
            persister: self,
        }
    }

    fn push(&mut self, update: Update) {
        let _ = self.write(update).expect("remove"); // TODO method should return result
        self.next = self.next.clone() + 1;
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Default, Clone)]
struct Counter(usize);

impl Display for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:0>12}", self.0)
    }
}
impl FromStr for Counter {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 12 {
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
    use crate::Update;

    use super::{Counter, FsPersister, Persister};

    struct MemoryPersister(Vec<Update>);
    impl MemoryPersister {
        pub fn new() -> Self {
            Self(vec![])
        }
    }
    impl Persister for MemoryPersister {
        fn iter(&self) -> impl ExactSizeIterator<Item = Update> {
            self.0.iter().cloned()
        }

        fn push(&mut self, update: crate::Update) {
            self.0.push(update)
        }
    }

    pub fn inner_test_persister<P: Persister>(mut persister: P, first_time: bool) {
        if first_time {
            assert_eq!(persister.iter().len(), 0);
        }

        let update1 = Update::deserialize(&lwk_test_util::update_test_vector_bytes()).unwrap();
        let update2 = {
            let mut update2 = update1.clone();
            update2.timestamps.push((22, 55));
            update2
        };
        assert_ne!(&update1, &update2);

        if first_time {
            persister.push(update1.clone());
            let mut iter = persister.iter();
            assert_eq!(iter.len(), 1);
            assert_eq!(iter.next(), Some(update1.clone()));
            drop(iter);

            persister.push(update2.clone());
        }
        let mut iter = persister.iter();
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(update1));
        assert_eq!(iter.next(), Some(update2));
    }

    #[test]
    fn test_memory_persister() {
        let persister = MemoryPersister::new();
        inner_test_persister(persister, true);
    }

    #[test]
    fn test_fs_persister() {
        let tempdir = tempfile::tempdir().unwrap();
        let persister = FsPersister::new(&tempdir).unwrap();
        inner_test_persister(persister, true);
        let persister = FsPersister::new(&tempdir).unwrap();
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
