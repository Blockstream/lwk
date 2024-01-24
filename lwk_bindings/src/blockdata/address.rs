use crate::{LwkError, Script};
use std::{fmt::Display, sync::Arc};

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct Address {
    inner: elements::Address,
}

impl From<elements::Address> for Address {
    fn from(inner: elements::Address) -> Self {
        Self { inner }
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[uniffi::export]
impl Address {
    /// Construct an Address object
    #[uniffi::constructor]
    pub fn new(s: &str) -> Result<Arc<Self>, LwkError> {
        let inner: elements::Address = s.parse()?;
        Ok(Arc::new(Self { inner }))
    }

    pub fn script_pubkey(&self) -> Arc<Script> {
        Arc::new(self.inner.script_pubkey().into())
    }

    pub fn is_blinded(&self) -> bool {
        self.inner.is_blinded()
    }

    pub fn to_unconfidential(&self) -> Arc<Self> {
        Arc::new(self.inner.to_unconfidential().into())
    }
}

#[cfg(test)]
mod tests {

    use super::Address;

    #[test]
    fn address() {
        let address_str = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";

        let address = Address::new(address_str).unwrap();
        assert_eq!(address.to_string(), address_str);

        assert_eq!(
            address.script_pubkey().to_string(),
            "0014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1"
        );

        assert!(address.is_blinded());

        assert_eq!(
            address.to_unconfidential().to_string(),
            "tex1q6rz28mcfaxtmd6v789l9rrlrusdprr9p634wu8"
        );
    }
}
