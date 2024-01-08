use std::{fmt, str::FromStr, sync::Arc};

use crate::Error;

/// The output descriptors
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct WolletDescriptor {
    inner: wollet::WolletDescriptor,
}

impl WolletDescriptor {
    #[uniffi::constructor]
    pub fn new(descriptor: String) -> Result<Arc<Self>, Error> {
        let inner = wollet::WolletDescriptor::from_str(&descriptor)?;
        Ok(Arc::new(WolletDescriptor { inner }))
    }
}

#[uniffi::export]
pub fn singlesig_desc_from_mnemonic(mnemonic: String) -> Result<Arc<WolletDescriptor>, Error> {
    // uniffi doesn't support associated function
    let signer = match signer::SwSigner::new(&mnemonic) {
        Ok(result) => result,
        Err(e) => return Err(Error::Generic { msg: e.to_string() }),
    };
    let script_variant = common::Singlesig::Wpkh;
    let blinding_variant = common::DescriptorBlindingKey::Slip77;
    let desc_str = match common::singlesig_desc(&signer, script_variant, blinding_variant) {
        Ok(result) => result,
        Err(e) => return Err(Error::Generic { msg: e.to_string() }),
    };
    WolletDescriptor::new(desc_str)
}

impl fmt::Display for WolletDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}
