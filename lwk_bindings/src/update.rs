use crate::LwkError;

/// Wrapper over [`lwk_wollet::Update`]
#[derive(uniffi::Object, Clone, PartialEq, Eq, Debug)]
pub struct Update {
    inner: lwk_wollet::Update,
}

impl From<lwk_wollet::Update> for Update {
    fn from(inner: lwk_wollet::Update) -> Self {
        Self { inner }
    }
}

impl From<Update> for lwk_wollet::Update {
    fn from(value: Update) -> Self {
        value.inner
    }
}

impl From<&Update> for lwk_wollet::Update {
    fn from(value: &Update) -> Self {
        value.inner.clone()
    }
}

impl AsRef<lwk_wollet::Update> for Update {
    fn as_ref(&self) -> &lwk_wollet::Update {
        &self.inner
    }
}

#[uniffi::export]
impl Update {
    /// Creates an `Update` from a byte array created with `serialize()`
    #[uniffi::constructor]
    pub fn new(bytes: &[u8]) -> Result<Update, LwkError> {
        Ok(lwk_wollet::Update::deserialize(bytes)?.into())
    }

    /// Serialize an `Update` to a byte array, can be deserialized back with `new()`
    pub fn serialize(&self) -> Result<Vec<u8>, LwkError> {
        Ok(self.inner.serialize()?)
    }

    /// Whether the update only changes the tip (does not affect transactions)
    pub fn only_tip(&self) -> bool {
        self.inner.only_tip()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn update() {
        let bytes = lwk_test_util::update_test_vector_bytes();
        let update = crate::Update::new(&bytes).unwrap();
        let back = update.serialize().unwrap();
        let update_back = crate::Update::new(&back).unwrap(); // now we save the version, thus we serialize back exactly the same

        assert_eq!(bytes.len(), back.len());
        assert_eq!(update, update_back);

        let update_v1 = {
            let mut update = update.clone();
            update.inner.version = 1;
            update
        };
        let back_v1 = update_v1.serialize().unwrap();
        let update_back_v1: crate::Update = crate::Update::new(&back_v1).unwrap();
        assert_eq!(update_v1, update_back_v1);

        assert_ne!(bytes, back_v1);
        assert_eq!(bytes.len() + 8, back_v1.len()); // the new version serialize the wallet status

        let update_v2 = {
            let mut update = update.clone();
            update.inner.version = 2;
            update
        };
        let _bytes = update_v2.serialize().unwrap();
    }
}
