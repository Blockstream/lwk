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
    #[uniffi::constructor]
    pub fn new(bytes: &[u8]) -> Result<Update, LwkError> {
        Ok(lwk_wollet::Update::deserialize(bytes)?.into())
    }

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
        let update_v0 = crate::Update::new(&bytes).unwrap();
        let back = update_v0.serialize().unwrap();
        let update_v1 = crate::Update::new(&back).unwrap();

        assert_ne!(bytes, back);
        assert_eq!(bytes.len() + 8, back.len()); // the new version serialize the wallet status

        assert_eq!(update_v0, update_v1);
    }
}
