use crate::{Error, Mnemonic, Network, Pset, WolletDescriptor};
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use wasm_bindgen::prelude::*;

/// A Software signer
#[wasm_bindgen]
pub struct Signer {
    inner: lwk_signer::SwSigner,
}

#[wasm_bindgen]
impl Signer {
    /// Construct a software signer
    pub fn new(mnemonic: &Mnemonic, network: &Network) -> Result<Signer, Error> {
        let inner = lwk_signer::SwSigner::new(&mnemonic.to_string(), network.is_mainnet())?;
        Ok(Self { inner })
    }

    pub fn sign(&self, pset: Pset) -> Result<Pset, Error> {
        let mut pset: PartiallySignedTransaction = pset.into();
        lwk_common::Signer::sign(&self.inner, &mut pset)?;
        Ok(pset.into())
    }

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
}

#[cfg(test)]
mod tests {
    use crate::{Mnemonic, Pset, Signer};
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
    }
}
