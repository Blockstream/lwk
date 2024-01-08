use std::{fmt::Display, str::FromStr, sync::Arc};

use signer::bip39;

use crate::Error;

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct Mnemonic {
    inner: bip39::Mnemonic,
}

impl From<bip39::Mnemonic> for Mnemonic {
    fn from(inner: bip39::Mnemonic) -> Self {
        Self { inner }
    }
}

impl Display for Mnemonic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl Mnemonic {
    /// Construct a Script object
    #[uniffi::constructor]
    pub fn new(s: String) -> Result<Arc<Self>, Error> {
        let inner = bip39::Mnemonic::from_str(&s)?;
        Ok(Arc::new(Self { inner }))
    }
}

#[cfg(test)]
mod tests {
    use crate::Mnemonic;

    #[test]
    fn mnemonic() {
        let mnemonic_str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::new(mnemonic_str.to_string()).unwrap();
        assert_eq!(mnemonic_str, mnemonic.to_string());
    }
}
