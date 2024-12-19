use std::collections::HashMap;

use crate::{Bip, Error, Mnemonic, Network, Pset, WolletDescriptor, Xpub};
use lwk_wollet::{
    bitcoin::bip32, elements::pset::PartiallySignedTransaction, elements_miniscript::slip77,
};
use wasm_bindgen::prelude::*;

/// A Software signer, wrapper of [`lwk_signer::SwSigner`]
#[wasm_bindgen]
pub struct Signer {
    inner: lwk_signer::SwSigner,
}

#[wasm_bindgen]
impl Signer {
    /// Creates a `Signer`
    #[wasm_bindgen(constructor)]
    pub fn new(mnemonic: &Mnemonic, network: &Network) -> Result<Signer, Error> {
        let inner = lwk_signer::SwSigner::new(&mnemonic.to_string(), network.is_mainnet())?;
        Ok(Self { inner })
    }

    /// Sign and consume the given PSET, returning the signed one
    pub fn sign(&self, pset: Pset) -> Result<Pset, Error> {
        let mut pset: PartiallySignedTransaction = pset.into();
        let added = lwk_common::Signer::sign(&self.inner, &mut pset)?;
        if added == 0 {
            return Err(Error::Generic("No signature added".to_string()));
        }
        Ok(pset.into())
    }

    #[wasm_bindgen(js_name = wpkhSlip77Descriptor)]
    pub fn wpkh_slip77_descriptor(&self) -> Result<WolletDescriptor, Error> {
        // TODO: make script_variant and blinding_variant parameters

        let is_mainnet = lwk_common::Signer::is_mainnet(&self.inner)?;
        let script_variant = lwk_common::Singlesig::Wpkh;
        let blinding_variant = lwk_common::DescriptorBlindingKey::Slip77;
        let desc_str =
            lwk_common::singlesig_desc(&self.inner, script_variant, blinding_variant, is_mainnet)
                .map_err(Error::Generic)?;

        WolletDescriptor::new(&desc_str)
    }

    #[wasm_bindgen(js_name = getMasterXpub)]
    pub fn get_master_xpub(&self) -> Result<Xpub, Error> {
        Ok(self.inner.xpub().into())
    }

    #[wasm_bindgen(js_name = keyoriginXpub)]
    pub fn keyorigin_xpub(&self, bip: Bip) -> Result<String, Error> {
        Ok(lwk_common::Signer::keyorigin_xpub(
            &self.inner,
            bip.into(),
            self.inner.is_mainnet(),
        )?)
    }

    pub fn mnemonic(&self) -> Mnemonic {
        self.inner
            .mnemonic()
            .expect("wasm bindings always create signer via mnemonic and not via xpriv")
            .into()
    }
}

#[allow(dead_code)]
// Used internally to emulate a sync signer for some methods
pub(crate) struct FakeSigner {
    pub(crate) paths: HashMap<bip32::DerivationPath, bip32::Xpub>,
    pub(crate) slip77: slip77::MasterBlindingKey,
}

impl lwk_common::Signer for FakeSigner {
    type Error = String;

    fn sign(&self, _pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        unimplemented!()
    }

    fn derive_xpub(&self, path: &bip32::DerivationPath) -> Result<bip32::Xpub, Self::Error> {
        self.paths
            .get(path)
            .cloned()
            .ok_or("Should contain all needed derivations".to_string())
    }

    fn slip77_master_blinding_key(&self) -> Result<slip77::MasterBlindingKey, Self::Error> {
        Ok(self.slip77)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use crate::{Bip, Mnemonic, Pset, Signer};
    use lwk_wollet::elements;
    use std::str::FromStr;
    use wasm_bindgen_test::*;

    pub fn regtest_policy_asset() -> elements::AssetId {
        elements::AssetId::from_str(
            "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225",
        )
        .unwrap()
    }

    pub fn network_regtest() -> lwk_wollet::ElementsNetwork {
        let policy_asset = regtest_policy_asset();
        lwk_wollet::ElementsNetwork::ElementsRegtest { policy_asset }
    }

    #[wasm_bindgen_test]
    fn signer() {
        let mnemonic_str =  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::new(mnemonic_str).unwrap();
        let network: crate::Network = network_regtest().into();

        let signer = Signer::new(&mnemonic, &network).unwrap();

        let pset_string =
            include_str!("../../lwk_jade/test_data/pset_to_be_signed.base64").to_string();
        let pset = Pset::new(&pset_string).unwrap();

        let signed_pset = signer.sign(pset.clone()).unwrap();

        assert_ne!(pset, signed_pset);

        assert_eq!(signer.get_master_xpub().unwrap().fingerprint(), "73c5da0a");

        assert_eq!(signer.keyorigin_xpub(Bip::bip49()).unwrap(), "[73c5da0a/49h/1h/0h]tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2");

        assert_eq!(signer.mnemonic(), mnemonic);
    }
}
