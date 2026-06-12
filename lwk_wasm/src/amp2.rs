use crate::{Error, Pset, WolletDescriptor};

use wasm_bindgen::prelude::*;

/// Context for actions interacting with Asset Management Platform version 2
#[wasm_bindgen]
pub struct Amp2 {
    inner: lwk_wollet::amp2::Amp2,
}

/// An Asset Management Platform version 2 descriptor
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
    /// Return the descriptor as a `WolletDescriptor`
    pub fn descriptor(&self) -> WolletDescriptor {
        self.inner.descriptor().into()
    }

    /// Return the string representation of the descriptor.
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    /// Create an `Amp2Descriptor` using any `WolletDescriptor`
    ///
    /// Warning: AMP2 server only supports a limited subset of descriptors.
    /// To make sure this AMP2 descriptor can be used safely,
    /// register this with AMP2 as soon as possible.
    #[wasm_bindgen(js_name = newWithCustomDescriptor)]
    pub fn new_with_custom_descriptor(desc: &WolletDescriptor) -> Self {
        let inner = lwk_wollet::amp2::Amp2Descriptor::new_with_custom_descriptor(desc.into());
        Self { inner }
    }
}

impl Amp2Descriptor {
    pub(crate) fn inner(&self) -> lwk_wollet::amp2::Amp2Descriptor {
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
    /// Create a new AMP2 client
    ///
    ///  * `server_key` - The keyorigin xpub of the AMP2 server key
    ///  * `url` - The URL of the AMP2 server
    #[wasm_bindgen(js_name = new)]
    pub fn new(server_key: &str, url: &str) -> Result<Self, Error> {
        let inner = lwk_wollet::amp2::Amp2::new(server_key.into(), url.into())?;
        Ok(Self { inner })
    }

    /// Create a new AMP2 client with the default url and server key for the testnet network.
    #[wasm_bindgen(js_name = newTestnet)]
    pub fn new_testnet() -> Self {
        let inner = lwk_wollet::amp2::Amp2::new_testnet();
        Self { inner }
    }

    /// Get an AMP2 wallet descriptor from the keyorigin xpub string obtained from a signer
    #[wasm_bindgen(js_name = descriptorFromStr)]
    pub fn descriptor_from_str(
        &self,
        keyorigin_xpub: &str,
        descriptor_blinding_key: &str,
    ) -> Result<Amp2Descriptor, Error> {
        Ok(self
            .inner
            .descriptor_from_str(keyorigin_xpub, descriptor_blinding_key)?
            .into())
    }

    /// Register an AMP2 wallet with the AMP2 server
    pub async fn register(&self, desc: &Amp2Descriptor) -> Result<String, Error> {
        Ok(self.inner.register(desc.inner()).await?.wid)
    }

    /// Ask the AMP2 server to cosign a PSET
    pub async fn cosign(&self, pset: &Pset) -> Result<Pset, Error> {
        let pset = self.inner.cosign(&pset.clone().into()).await?.pset;
        Ok(pset.into())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_amp2() {
        let expected = "ct(slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67),elwsh(multi(2,[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd/<0;1>/*,[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj/<0;1>/*)))#6j2fne4s";
        let descriptor_blinding_key =
            "slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67)";
        let k = "[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj";
        let amp2 = Amp2::new_testnet();
        let d = amp2
            .descriptor_from_str(k, descriptor_blinding_key)
            .unwrap();
        assert_eq!(d.descriptor().to_string(), expected);

        let server_key = "[3d970d04/87h/1h/0h]tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd";
        let url = "http://127.0.0.1:5000";
        let amp2 = Amp2::new(server_key.into(), url.into()).unwrap();
        let d = amp2
            .descriptor_from_str(k, descriptor_blinding_key)
            .unwrap();
        assert_eq!(d.descriptor().to_string(), expected);

        let custom_desc = "ct(1111111111111111111111111111111111111111111111111111111111111111,elwsh(and_v(v:pk(026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3),multi(2,[342c8926/87h/1h/0h]tpubDDWUA7YvBHxdurKUrYFkdjsB59koHqvGRJ3j9zDhwMycxXHXz1ujTfHMB66K4rEWDM8BoDKDdJx3rVGp2qUSPnXVpQXi8qtnXqa96nPnZAH/0/*,[af9e5bc2/87h/1h/0h]tpubDDRPayLs2vBkRkyQ9X2BEhojxCy9vvZpjhubEVosz5pi66LuuAuyZQiUtsPBN5wSfhWLoMYM3gqVqT3Po4GpcWGUfPh8514ZBB9hfWFNEUA/0/*,[57411aec/87h/1h/0h]tpubDDmweWcTcRb54kZqy3Gv5JF8SjAyuoK3uPYXp24uz6nfsKjJojxjdZAang5HXDmtS8tg5CJntUC4fzn4aY5Dsg6Aphvq42vK9edmgX83NFg/0/*))))";
        let wd = WolletDescriptor::new(custom_desc).unwrap();
        let _amp2_desc = Amp2Descriptor::new_with_custom_descriptor(&wd);
    }
}
