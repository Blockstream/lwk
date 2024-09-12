use crate::{LwkError, Mnemonic, Network, Pset, WolletDescriptor};
use std::sync::Arc;

/// wrapper over [`lwk_common::Bip`]
#[derive(uniffi::Object)]
pub struct Bip {
    inner: lwk_common::Bip,
}

#[uniffi::export]
impl Bip {
    /// For P2SH-P2WPKH wallets
    #[uniffi::constructor]
    pub fn new_bip49() -> Self {
        Self {
            inner: lwk_common::Bip::Bip49,
        }
    }

    /// For P2WPKH wallets
    #[uniffi::constructor]
    pub fn new_bip84() -> Self {
        Self {
            inner: lwk_common::Bip::Bip84,
        }
    }

    /// For multisig wallets
    #[uniffi::constructor]
    pub fn new_bip87() -> Self {
        Self {
            inner: lwk_common::Bip::Bip87,
        }
    }
}

/// A Software signer, wrapper over [`lwk_signer::SwSigner`]
#[derive(uniffi::Object)]
pub struct Signer {
    inner: lwk_signer::SwSigner,
}

#[uniffi::export]
impl Signer {
    /// Construct a software signer
    #[uniffi::constructor]
    pub fn new(mnemonic: &Mnemonic, network: &Network) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_signer::SwSigner::new(&mnemonic.to_string(), network.is_mainnet())?;
        Ok(Arc::new(Self { inner }))
    }

    /// Sign the given `pset`
    ///
    /// Note from an API perspective it would be better to consume the `pset` parameter so it would
    /// be clear the signed PSET is the returned one, but it's not possible with uniffi bindings
    pub fn sign(&self, pset: &Pset) -> Result<Arc<Pset>, LwkError> {
        let mut pset = pset.inner();
        lwk_common::Signer::sign(&self.inner, &mut pset)?;
        Ok(Arc::new(pset.into()))
    }

    pub fn wpkh_slip77_descriptor(&self) -> Result<Arc<WolletDescriptor>, LwkError> {
        // TODO: make script_variant and blinding_variant parameters

        let is_mainnet = lwk_common::Signer::is_mainnet(&self.inner)?;
        let script_variant = lwk_common::Singlesig::Wpkh;
        let blinding_variant = lwk_common::DescriptorBlindingKey::Slip77;
        let desc_str =
            lwk_common::singlesig_desc(&self.inner, script_variant, blinding_variant, is_mainnet)?;

        WolletDescriptor::new(&desc_str)
    }
}

#[cfg(test)]
mod tests {
    use lwk_wollet::ElementsNetwork;

    use crate::{Mnemonic, Pset, Signer};

    #[test]
    fn signer() {
        let mnemonic_str = lwk_test_util::TEST_MNEMONIC;
        let mnemonic = Mnemonic::new(mnemonic_str).unwrap();
        let network: crate::Network = ElementsNetwork::default_regtest().into();

        let signer = Signer::new(&mnemonic, &network).unwrap();

        let pset_string =
            include_str!("../../lwk_jade/test_data/pset_to_be_signed.base64").to_string();
        let pset = Pset::new(&pset_string).unwrap();

        let signed_pset = signer.sign(&pset).unwrap();

        assert_ne!(pset, signed_pset);
    }
}
