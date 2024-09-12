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

    /// Generate a new random software signer
    #[uniffi::constructor]
    pub fn random(network: &Network) -> Result<Arc<Self>, LwkError> {
        let (inner, _mnemonic) = lwk_signer::SwSigner::random(network.is_mainnet())?;
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

    pub fn keyorigin_xpub(&self, bip: &Bip) -> Result<String, LwkError> {
        let is_mainnet = lwk_common::Signer::is_mainnet(&self.inner)?;
        Ok(lwk_common::Signer::keyorigin_xpub(
            &self.inner,
            bip.inner,
            is_mainnet,
        )?)
    }

    pub fn mnemonic(&self) -> Result<Arc<Mnemonic>, LwkError> {
        Ok(Arc::new(self.inner.mnemonic().map(Into::into).ok_or_else(
            || LwkError::Generic {
                msg: "Mnemonic not available".to_string(),
            },
        )?))
    }
}

#[cfg(test)]
mod tests {
    use lwk_wollet::ElementsNetwork;

    use crate::{Bip, Mnemonic, Pset, Signer};

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

        let xpub = signer.keyorigin_xpub(&Bip::new_bip49()).unwrap();
        let expected = "[73c5da0a/49h/1h/0h]tpubDD7tXK8KeQ3YY83yWq755fHY2JW8Ha8Q765tknUM5rSvjPcGWfUppDFMpQ1ScziKfW3ZNtZvAD7M3u7bSs7HofjTD3KP3YxPK7X6hwV8Rk2";
        assert_eq!(xpub, expected);

        let xpub = signer.keyorigin_xpub(&Bip::new_bip84()).unwrap();
        let expected = "[73c5da0a/84h/1h/0h]tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M";
        assert_eq!(xpub, expected);

        let xpub = signer.keyorigin_xpub(&Bip::new_bip87()).unwrap();
        let expected = "[73c5da0a/87h/1h/0h]tpubDCChhoz5Qdrkn7Z7KXawq6Ad6r3A4MUkCoVTqeWxfTkA6bHNJ3CHUEtALQdkNeixNz4446PcAmw4WKcj3mV2vb29H7sg9EPzbyCU1y2merw";
        assert_eq!(xpub, expected);

        assert_eq!(signer.mnemonic().unwrap(), mnemonic);
    }
}
