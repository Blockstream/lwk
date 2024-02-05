use crate::LwkError;

#[derive(uniffi::Object, Clone, PartialEq, Eq)]
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
}

#[cfg(test)]
mod tests {

    #[test]
    fn update() {
        let bytes = lwk_test_util::update_test_vector_bytes();
        let update = crate::Update::new(&bytes).unwrap();
        assert_eq!(update.serialize().unwrap(), bytes);
    }
}
