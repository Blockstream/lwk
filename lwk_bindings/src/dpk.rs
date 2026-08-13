use std::{fmt, str::FromStr, sync::Arc};

use crate::LwkError;

/// A descriptor public key
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct DescriptorPublicKey {
    inner: lwk_common::DescriptorPublicKey,
}

impl fmt::Display for DescriptorPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl DescriptorPublicKey {
    /// Construct a DescriptorPublicKey from its string representation
    ///
    /// Accepts both a bare xpub and a keyorigin xpub
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_common::DescriptorPublicKey::from_str(s)?;
        Ok(Arc::new(Self { inner }))
    }
}
