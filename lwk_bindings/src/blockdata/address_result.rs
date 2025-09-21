//! Liquid address result

use crate::Address;
use std::sync::Arc;

/// Value returned from asking an address to the wallet.
/// Containing the confidential address and its
/// derivation index (the last element in the derivation path)
#[derive(uniffi::Object)]
pub struct AddressResult {
    inner: lwk_wollet::AddressResult,
}

impl From<lwk_wollet::AddressResult> for AddressResult {
    fn from(inner: lwk_wollet::AddressResult) -> Self {
        Self { inner }
    }
}

#[uniffi::export]
impl AddressResult {
    /// Return the address.
    pub fn address(&self) -> Arc<Address> {
        Arc::new(self.inner.address().clone().into())
    }

    /// Return the derivation index of the address.
    pub fn index(&self) -> u32 {
        self.inner.index()
    }
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use super::AddressResult;

    #[test]
    fn address_result() {
        let address_str = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
        let index = 0;
        let wollet_address_result = lwk_wollet::AddressResult::new(
            elements::Address::from_str(address_str).unwrap(),
            index,
        );

        let address_result: AddressResult = wollet_address_result.into();

        assert_eq!(address_result.address().to_string(), address_str);

        assert_eq!(address_result.index(), index);
    }
}
