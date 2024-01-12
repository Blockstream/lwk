use std::{fmt::Display, sync::Arc};

#[derive(uniffi::Object, Debug, Clone)]
#[uniffi::export(Display)]
pub struct ElectrumUrl {
    pub(crate) url: String,
    pub(crate) tls: bool,
    pub(crate) validate_domain: bool,
}

impl Display for ElectrumUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[uniffi::export]
impl ElectrumUrl {
    /// Construct a Script object
    #[uniffi::constructor]
    pub fn new(electrum_url: String, tls: bool, validate_domain: bool) -> Arc<Self> {
        Arc::new(Self {
            url: electrum_url,
            tls,
            validate_domain,
        })
    }
}
