#[derive(uniffi::Object)]
pub struct ExternalUtxo {
    inner: lwk_wollet::ExternalUtxo,
}

impl From<lwk_wollet::ExternalUtxo> for ExternalUtxo {
    fn from(inner: lwk_wollet::ExternalUtxo) -> Self {
        Self { inner }
    }
}

impl From<&ExternalUtxo> for lwk_wollet::ExternalUtxo {
    fn from(value: &ExternalUtxo) -> Self {
        value.inner.clone()
    }
}

// TODO: method to inspect inner
