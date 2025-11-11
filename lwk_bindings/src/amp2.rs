use std::sync::Arc;

use crate::{LwkError, Pset, WolletDescriptor};

/// Wrapper over [`lwk_wollet::amp2::Amp2`]
#[derive(uniffi::Object)]
pub struct Amp2 {
    inner: lwk_wollet::amp2::Amp2,
}

/// Wrapper over [`lwk_wollet::amp2::Amp2Descriptor`]
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct Amp2Descriptor {
    inner: lwk_wollet::amp2::Amp2Descriptor,
}

impl std::fmt::Display for Amp2Descriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

#[uniffi::export]
impl Amp2Descriptor {
    pub fn descriptor(&self) -> WolletDescriptor {
        self.inner.descriptor().into()
    }
}

impl From<lwk_wollet::amp2::Amp2Descriptor> for Amp2Descriptor {
    fn from(inner: lwk_wollet::amp2::Amp2Descriptor) -> Self {
        Self { inner }
    }
}

impl From<Amp2Descriptor> for lwk_wollet::amp2::Amp2Descriptor {
    fn from(desc: Amp2Descriptor) -> Self {
        desc.inner.clone()
    }
}

impl From<&Amp2Descriptor> for lwk_wollet::amp2::Amp2Descriptor {
    fn from(desc: &Amp2Descriptor) -> Self {
        desc.inner.clone()
    }
}

#[uniffi::export]
impl Amp2 {
    /// Construct an AMP2 context for Liquid Testnet
    #[uniffi::constructor]
    pub fn new_testnet() -> Self {
        let inner = lwk_wollet::amp2::Amp2::new_testnet();
        Self { inner }
    }

    /// Create an AMP2 wallet descriptor from the keyorigin xpub of a signer
    pub fn descriptor_from_str(&self, keyorigin_xpub: &str) -> Result<Amp2Descriptor, LwkError> {
        Ok(self.inner.descriptor_from_str(keyorigin_xpub)?.into())
    }

    // "register" is a reserved keyword in some target languages, do not use it
    /// Register an AMP2 wallet with the AMP2 server
    pub fn register_wallet(&self, desc: &Amp2Descriptor) -> Result<String, LwkError> {
        Ok(self.inner.blocking_register(desc.into())?.wid)
    }

    /// Ask the AMP2 server to cosign a PSET
    pub fn cosign(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let pset = self.inner.blocking_cosign(&pset.inner())?.pset;
        Ok(Arc::new(pset.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amp2() {
        let expected = "ct(slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67),elwsh(multi(2,[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd/<0;1>/*,[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj/<0;1>/*)))#6j2fne4s";
        let k = "[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj";
        let amp2 = Amp2::new_testnet();
        let d = amp2.descriptor_from_str(k).unwrap();
        assert_eq!(d.descriptor().to_string(), expected);
        // let _wid = amp2.register(&d).unwrap();
    }
}
