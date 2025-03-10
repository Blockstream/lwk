use crate::{Error, Pset, WolletDescriptor};

use wasm_bindgen::prelude::*;

/// Wrapper of [`lwk_wollet::amp2::Amp2`]
#[wasm_bindgen]
pub struct Amp2 {
    inner: lwk_wollet::amp2::Amp2,
}

/// Wrapper of [`lwk_wollet::amp2::Amp2Descriptor`]
#[wasm_bindgen]
pub struct Amp2Descriptor {
    inner: lwk_wollet::amp2::Amp2Descriptor,
}

impl std::fmt::Display for Amp2Descriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[wasm_bindgen]
impl Amp2Descriptor {
    pub fn descriptor(&self) -> WolletDescriptor {
        self.inner.descriptor().into()
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{}", self)
    }
}

impl Amp2Descriptor {
    pub fn inner(&self) -> lwk_wollet::amp2::Amp2Descriptor {
        self.inner.clone()
    }
}

impl From<lwk_wollet::amp2::Amp2Descriptor> for Amp2Descriptor {
    fn from(inner: lwk_wollet::amp2::Amp2Descriptor) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl Amp2 {
    pub fn new_testnet() -> Self {
        let inner = lwk_wollet::amp2::Amp2::new_testnet();
        Self { inner }
    }

    pub fn descriptor_from_str(&self, keyorigin_xpub: &str) -> Result<Amp2Descriptor, Error> {
        Ok(self.inner.descriptor_from_str(keyorigin_xpub)?.into())
    }

    pub async fn register(&self, desc: &Amp2Descriptor) -> Result<String, Error> {
        Ok(self.inner.register(desc.inner()).await?.wid)
    }

    pub async fn cosign(&self, pset: &Pset) -> Result<Pset, Error> {
        let pset = self.inner.cosign(&pset.clone().into()).await?.pset;
        Ok(pset.into())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    use crate::WolletDescriptor;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_amp2() {
        let expected = "ct(slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67),elwsh(multi(2,[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd/<0;1>/*,[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj/<0;1>/*)))#6j2fne4s";
        let k = "[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj";
        let amp2 = Amp2::new_testnet();
        let d = amp2.descriptor_from_str(k).unwrap();
        assert_eq!(d.descriptor().to_string(), expected);
    }
}
