#[derive(uniffi::Object, Clone)]
pub struct Update {
    inner: wollet::Update,
}

impl From<wollet::Update> for Update {
    fn from(inner: wollet::Update) -> Self {
        Self { inner }
    }
}

impl From<Update> for wollet::Update {
    fn from(value: Update) -> Self {
        value.inner
    }
}
