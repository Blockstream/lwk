#[derive(uniffi::Object, Clone)]
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
