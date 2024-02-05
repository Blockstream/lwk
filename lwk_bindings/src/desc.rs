use std::{fmt, str::FromStr, sync::Arc};

use crate::LwkError;

/// The output descriptors
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct WolletDescriptor {
    inner: lwk_wollet::WolletDescriptor,
}

impl AsRef<lwk_wollet::WolletDescriptor> for WolletDescriptor {
    fn as_ref(&self) -> &lwk_wollet::WolletDescriptor {
        &self.inner
    }
}

impl From<lwk_wollet::WolletDescriptor> for WolletDescriptor {
    fn from(inner: lwk_wollet::WolletDescriptor) -> Self {
        Self { inner }
    }
}

impl From<&WolletDescriptor> for lwk_wollet::WolletDescriptor {
    fn from(desc: &WolletDescriptor) -> Self {
        desc.inner.clone()
    }
}

#[uniffi::export]
impl WolletDescriptor {
    #[uniffi::constructor]
    pub fn new(descriptor: &str) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_wollet::WolletDescriptor::from_str(descriptor)?;
        Ok(Arc::new(WolletDescriptor { inner }))
    }
}

impl fmt::Display for WolletDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Mnemonic, Signer, WolletDescriptor};
    use std::str::FromStr;

    #[test]
    fn wpkh_slip77_descriptor() {
        let mnemonic_str = lwk_test_util::TEST_MNEMONIC;
        let mnemonic = Mnemonic::new(mnemonic_str).unwrap();
        let network: crate::Network = lwk_test_util::network_regtest().into();

        let signer = Signer::new(&mnemonic, &network).unwrap();
        let exp = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d";
        assert_eq!(signer.wpkh_slip77_descriptor().unwrap().to_string(), exp);

        let wollet_desc = lwk_wollet::WolletDescriptor::from_str(exp).unwrap();
        let desc: WolletDescriptor = wollet_desc.into();
        assert_eq!(desc.to_string(), exp);
    }
}
