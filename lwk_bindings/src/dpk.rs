use std::{fmt, str::FromStr, sync::Arc};

use crate::{DerivationPath, LwkError};

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

impl AsRef<lwk_common::DescriptorPublicKey> for DescriptorPublicKey {
    fn as_ref(&self) -> &lwk_common::DescriptorPublicKey {
        &self.inner
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

    /// Return the fingerprint of the master key, if key origin information is available.
    pub fn fingerprint(&self) -> Option<String> {
        self.inner.fingerprint().map(|f| f.to_string())
    }

    /// Return the extended public key, without any key origin information.
    pub fn xpub(&self) -> Option<String> {
        self.inner.xpub().map(|f| f.to_string())
    }

    /// Return the derivation path from the master key, if key origin information is available.
    pub fn derivation_path(&self) -> Option<Arc<DerivationPath>> {
        self.inner
            .derivation_path()
            .map(|p| Arc::new(p.clone().into()))
    }
}
