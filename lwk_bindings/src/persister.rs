use std::sync::Arc;

use lwk_wollet::Persister;

use crate::{LwkError, Update};

#[uniffi::export(with_foreign)]
pub trait ForeignPersister: Send + Sync {
    fn get(&self, index: u64) -> Result<Option<Arc<Update>>, LwkError>;
    fn push(&self, update: Arc<Update>) -> Result<(), LwkError>;
}

impl From<LwkError> for lwk_wollet::PersistError {
    fn from(e: LwkError) -> Self {
        lwk_wollet::PersistError::Other(format!("{e:?}"))
    }
}

#[derive(uniffi::Object)]
pub struct ForeignPersisterLink {
    pub(crate) inner: Arc<dyn ForeignPersister>,
}

#[uniffi::export]
impl ForeignPersisterLink {
    #[uniffi::constructor]
    pub fn new(persister: Arc<dyn ForeignPersister>) -> Self {
        Self { inner: persister }
    }
}

impl Persister for ForeignPersisterLink {
    fn push(&self, update: lwk_wollet::Update) -> Result<(), lwk_wollet::PersistError> {
        self.inner.push(Arc::new(update.into()))?;
        Ok(())
    }

    fn get(&self, index: usize) -> Result<Option<lwk_wollet::Update>, lwk_wollet::PersistError> {
        Ok(self
            .inner
            .get(index as u64)
            .map(|r| r.map(|o| o.as_ref().into()))?)
    }
}
