use lwk_wollet::elements;
use crate::Error;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Address {
    inner: elements::Address,
}

impl From<elements::Address> for Address {
    fn from(inner: elements::Address) -> Self {
        Self { inner }
    }
}

impl AsRef<elements::Address> for Address {
    fn as_ref(&self) -> &elements::Address {
        &self.inner
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Address {
    /// Construct an Address object
    pub fn new(s: &str) -> Result<Address, Error> {
        let inner: elements::Address = s.parse()?;
        Ok(inner.into())
    }

    pub fn is_blinded(&self) -> bool {
        self.inner.is_blinded()
    }

    pub fn to_unconfidential(&self) -> Address {
        self.inner.to_unconfidential().into()
    }
}

#[cfg(test)]
mod tests {

    use super::Address;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn address() {
        let address_str = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";

        let address = Address::new(address_str).unwrap();
        assert_eq!(address.to_string(), address_str);

        assert!(address.is_blinded());

        assert_eq!(
            address.to_unconfidential().to_string(),
            "tex1q6rz28mcfaxtmd6v789l9rrlrusdprr9p634wu8"
        );
    }
}
