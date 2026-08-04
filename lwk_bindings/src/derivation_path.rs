use crate::{LwkError, Network};

use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

use elements::bitcoin::bip32::{self, ChildNumber};

/// A BIP32 derivation path
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone)]
#[uniffi::export(Display)]
pub struct DerivationPath {
    inner: bip32::DerivationPath,
}

impl From<bip32::DerivationPath> for DerivationPath {
    fn from(inner: bip32::DerivationPath) -> Self {
        Self { inner }
    }
}

impl From<DerivationPath> for bip32::DerivationPath {
    fn from(value: DerivationPath) -> Self {
        value.inner
    }
}

impl From<&DerivationPath> for bip32::DerivationPath {
    fn from(value: &DerivationPath) -> Self {
        value.inner.clone()
    }
}

impl AsRef<bip32::DerivationPath> for DerivationPath {
    fn as_ref(&self) -> &bip32::DerivationPath {
        &self.inner
    }
}

impl Display for DerivationPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl DerivationPath {
    /// Construct a DerivationPath from its string representation
    ///
    /// For example: "m/84'/1'/0'" or "84h/1h/0h".
    #[uniffi::constructor]
    pub fn new(path: &str) -> Result<Arc<Self>, LwkError> {
        let inner = bip32::DerivationPath::from_str(path)?;
        Ok(Arc::new(Self { inner }))
    }

    /// Construct a DerivationPath from a vector of u32
    #[uniffi::constructor]
    pub fn from_vec(path: Vec<u32>) -> Arc<Self> {
        let inner = path.into_iter().map(ChildNumber::from).collect();
        Arc::new(Self { inner })
    }

    /// Construct the account-level derivation path
    ///
    /// `account_type` must be one of "wpkh", "shwpkh", "pkh" or "tr"
    #[uniffi::constructor]
    pub fn from_account(
        network: &Network,
        account_type: &str,
        account_num: u32,
    ) -> Result<Arc<Self>, LwkError> {
        let coin_type = if network.is_mainnet() { 1776 } else { 1 };
        let purpose = match account_type {
            "wpkh" => 84,
            "shwpkh" => 49,
            "pkh" => 44,
            "tr" => 86,
            _ => {
                return Err(LwkError::Generic {
                    msg: "invalid account type, must be 'wpkh', 'shwpkh', 'pkh' or 'tr'".into(),
                })
            }
        };
        let h: u32 = 1 << 31;
        if account_num >= h {
            return Err(LwkError::Generic {
                msg: "invalid account number".into(),
            });
        }

        Ok(Self::from_vec(vec![
            purpose + h,
            coin_type + h,
            account_num + h,
        ]))
    }

    /// Return the derivation path as a vector of u32
    pub fn to_vec(&self) -> Vec<u32> {
        self.inner.into_iter().map(|c| u32::from(*c)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::DerivationPath;

    #[test]
    fn test_derivation_path() {
        let s = "84'/1'/0'";
        let path = DerivationPath::new(s).unwrap();
        assert_eq!(path.to_string(), s);
        assert_eq!(path.to_vec(), vec![84 + (1 << 31), 1 + (1 << 31), 1 << 31]);

        let from_vec = DerivationPath::from_vec(path.to_vec());
        assert_eq!(from_vec.to_string(), s);
        assert_eq!(from_vec, path);

        assert!(DerivationPath::new("not a path").is_err());
    }
}
