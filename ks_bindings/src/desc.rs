use std::{fmt, str::FromStr, sync::Arc};

use crate::Error;

/// The output descriptors
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct WolletDescriptor {
    inner: wollet::WolletDescriptor,
}

impl From<wollet::WolletDescriptor> for WolletDescriptor {
    fn from(inner: wollet::WolletDescriptor) -> Self {
        Self { inner }
    }
}

impl WolletDescriptor {
    #[uniffi::constructor]
    pub fn new(descriptor: String) -> Result<Arc<Self>, Error> {
        let inner = wollet::WolletDescriptor::from_str(&descriptor)?;
        Ok(Arc::new(WolletDescriptor { inner }))
    }
}

impl fmt::Display for WolletDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}
