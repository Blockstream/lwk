use crate::{LwkError, Mnemonic, Network, Pset, WolletDescriptor};
use std::sync::Arc;

/// A Software signer
#[derive(uniffi::Object)]
pub struct Signer {
    inner: signer::SwSigner,
}

#[uniffi::export]
impl Signer {
    /// Construct a software signer
    #[uniffi::constructor]
    pub fn new(mnemonic: &Mnemonic, network: &Network) -> Result<Arc<Self>, LwkError> {
        let inner = signer::SwSigner::new(&mnemonic.to_string(), network.is_mainnet())?;
        Ok(Arc::new(Self { inner }))
    }

    pub fn sign(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let mut pset = pset.inner();
        common::Signer::sign(&self.inner, &mut pset)?;
        Ok(Arc::new(pset.into()))
    }

    pub fn wpkh_slip77_descriptor(&self) -> Result<Arc<WolletDescriptor>, LwkError> {
        // TODO: make script_variant and blinding_variant parameters

        let is_mainnet = common::Signer::is_mainnet(&self.inner)?;
        let script_variant = common::Singlesig::Wpkh;
        let blinding_variant = common::DescriptorBlindingKey::Slip77;
        let desc_str =
            common::singlesig_desc(&self.inner, script_variant, blinding_variant, is_mainnet)?;

        WolletDescriptor::new(desc_str)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Mnemonic, Pset, Signer};

    #[test]
    fn signer() {
        let mnemonic_str = test_util::TEST_MNEMONIC;
        let mnemonic = Mnemonic::new(mnemonic_str.to_string()).unwrap();
        let network: crate::Network = test_util::network_regtest().into();

        let signer = Signer::new(&mnemonic, &network).unwrap();

        let pset_string = include_str!("../../jade/test_data/pset_to_be_signed.base64").to_string();
        let pset = Pset::new(pset_string.clone()).unwrap();

        let signed_pset = signer.sign(&pset).unwrap();

        assert_ne!(pset, signed_pset);
    }
}
