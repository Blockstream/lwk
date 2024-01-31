use crate::Update;

trait Persister {
    /// Return the elements in the same order as they have been inserted
    fn iter(&self) -> impl ExactSizeIterator<Item = &Update>;

    /// Push and persist an update.
    ///
    /// Implementors could decide to override previous element if both previous element and current
    /// has `update.only_tip() == true`
    fn push(&mut self, update: Update);
}

#[cfg(test)]
mod test {
    use crate::Update;

    use super::Persister;

    struct MemoryPersister(Vec<Update>);
    impl MemoryPersister {
        pub fn new() -> Self {
            Self(vec![])
        }
    }
    impl Persister for MemoryPersister {
        fn iter(&self) -> impl ExactSizeIterator<Item = &Update> {
            self.0.iter()
        }

        fn push(&mut self, update: crate::Update) {
            self.0.push(update)
        }
    }

    pub fn inner_test_persister<P: Persister>(mut persister: P) {
        assert_eq!(persister.iter().len(), 0);

        let update1 = Update::deserialize(&lwk_test_util::update_test_vector_bytes()).unwrap();
        let update2 = {
            let mut update2 = update1.clone();
            update2.timestamps.push((22, 55));
            update2
        };
        assert_ne!(&update1, &update2);

        persister.push(update1.clone());
        let mut iter = persister.iter();
        assert_eq!(iter.len(), 1);
        assert_eq!(iter.next(), Some(&update1));
        drop(iter);

        persister.push(update2.clone());
        let mut iter = persister.iter();
        assert_eq!(iter.len(), 2);
        assert_eq!(iter.next(), Some(&update1));
        assert_eq!(iter.next(), Some(&update2));
    }

    #[test]
    fn test_persister() {
        let persister = MemoryPersister::new();
        inner_test_persister(persister);
    }
}
