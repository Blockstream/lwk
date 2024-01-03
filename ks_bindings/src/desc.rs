use std::{fmt, sync::Arc};

use crate::Error;

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct SingleSigCTDesc {
    val: String,
}

impl SingleSigCTDesc {
    pub fn as_str(&self) -> &str {
        &self.val
    }
}

#[uniffi::export]
impl SingleSigCTDesc {
    #[uniffi::constructor]
    pub fn new(mnemonic: String) -> Result<Arc<Self>, Error> {
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
        Ok(Arc::new(SingleSigCTDesc { val: desc_str }))
    }
}
impl fmt::Display for SingleSigCTDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.val)
    }
}
