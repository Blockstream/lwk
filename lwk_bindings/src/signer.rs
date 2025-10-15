use crate::{LwkError, Mnemonic, Network, Pset, WolletDescriptor};
use std::sync::Arc;

#[derive(uniffi::Enum)]
pub enum Singlesig {
    Wpkh,
    ShWpkh,
}

impl From<Singlesig> for lwk_common::Singlesig {
    fn from(singlesig: Singlesig) -> Self {
        match singlesig {
            Singlesig::Wpkh => lwk_common::Singlesig::Wpkh,
            Singlesig::ShWpkh => lwk_common::Singlesig::ShWpkh,
        }
    }
}

#[derive(uniffi::Enum)]
pub enum DescriptorBlindingKey {
    Slip77,
    Slip77Rand,
    Elip151,
}

impl From<DescriptorBlindingKey> for lwk_common::DescriptorBlindingKey {
    fn from(blinding_key: DescriptorBlindingKey) -> Self {
        match blinding_key {
            DescriptorBlindingKey::Slip77 => lwk_common::DescriptorBlindingKey::Slip77,
            DescriptorBlindingKey::Slip77Rand => lwk_common::DescriptorBlindingKey::Slip77Rand,
            DescriptorBlindingKey::Elip151 => lwk_common::DescriptorBlindingKey::Elip151,
        }
    }
}

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
    pub(crate) inner: lwk_signer::SwSigner,
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

    /// Return the witness public key hash, slip77 descriptor of this signer
    pub fn wpkh_slip77_descriptor(&self) -> Result<Arc<WolletDescriptor>, LwkError> {
        self.singlesig_desc(Singlesig::Wpkh, DescriptorBlindingKey::Slip77)
    }

    /// Generate a singlesig descriptor with the given parameters
    pub fn singlesig_desc(
        &self,
        script_variant: Singlesig,
        blinding_variant: DescriptorBlindingKey,
    ) -> Result<Arc<WolletDescriptor>, LwkError> {
        let desc_str = lwk_common::singlesig_desc(
            &self.inner,
            script_variant.into(),
            blinding_variant.into(),
        )?;
        WolletDescriptor::new(&desc_str)
    }

    /// Return keyorigin and xpub, like "[73c5da0a/84h/1h/0h]tpub..."
    pub fn keyorigin_xpub(&self, bip: &Bip) -> Result<String, LwkError> {
        let is_mainnet = lwk_common::Signer::is_mainnet(&self.inner)?;
        Ok(lwk_common::Signer::keyorigin_xpub(
            &self.inner,
            bip.inner,
            is_mainnet,
        )?)
    }

    /// Return the signer fingerprint
    pub fn fingerprint(&self) -> Result<String, LwkError> {
        Ok(self.inner.fingerprint().to_string())
    }

    /// Get the mnemonic of the signer
    pub fn mnemonic(&self) -> Result<Arc<Mnemonic>, LwkError> {
        Ok(Arc::new(self.inner.mnemonic().map(Into::into).ok_or_else(
            || LwkError::Generic {
                msg: "Mnemonic not available".to_string(),
            },
        )?))
    }

    /// Derive a BIP85 mnemonic from this signer
    ///
    /// # Arguments
    /// * `index` - The index for the derived mnemonic (0-based)
    /// * `word_count` - The number of words in the derived mnemonic (12 or 24)
    ///
    /// # Returns
    /// * `Ok(Mnemonic)` - The derived BIP85 mnemonic
    /// * `Err(LwkError)` - If BIP85 derivation fails
    ///
    /// # Example
    /// ```python
    /// signer = Signer.new(mnemonic, network)
    /// derived_mnemonic = signer.derive_bip85_mnemonic(0, 12)
    /// ```
    pub fn derive_bip85_mnemonic(
        &self,
        index: u32,
        word_count: u32,
    ) -> Result<Arc<Mnemonic>, LwkError> {
        let derived_mnemonic = self.inner.derive_bip85_mnemonic(index, word_count)?;
        Ok(Arc::new(derived_mnemonic.into()))
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

        assert_eq!(signer.fingerprint().unwrap(), xpub[1..9])
    }

    #[test]
    fn test_bip85_derivation() {
        let mnemonic_str = lwk_test_util::TEST_MNEMONIC;
        let mnemonic = Mnemonic::new(mnemonic_str).unwrap();
        let network: crate::Network = ElementsNetwork::default_regtest().into();

        let signer = Signer::new(&mnemonic, &network).unwrap();

        // Test BIP85 derivation with 12 words
        let derived_mnemonic_12 = signer.derive_bip85_mnemonic(0, 12).unwrap();
        assert_eq!(derived_mnemonic_12.word_count(), 12);
        let expected_mnemonic_12 =
            "prosper short ramp prepare exchange stove life snack client enough purpose fold";
        assert_eq!(derived_mnemonic_12.to_string(), expected_mnemonic_12);

        // Test BIP85 derivation with 24 words
        let derived_mnemonic_24 = signer.derive_bip85_mnemonic(0, 24).unwrap();
        assert_eq!(derived_mnemonic_24.word_count(), 24);
        let expected_mnemonic_24 = "stick exact spice sock filter ginger museum horse kit multiply manual wear grief demand derive alert quiz fault december lava picture immune decade jaguar";
        assert_eq!(derived_mnemonic_24.to_string(), expected_mnemonic_24);

        // Test that different indices produce different mnemonics
        let derived_mnemonic_1 = signer.derive_bip85_mnemonic(1, 12).unwrap();
        assert_ne!(
            derived_mnemonic_12.to_string(),
            derived_mnemonic_1.to_string()
        );

        // Test that the same index produces the same mnemonic
        let derived_mnemonic_0_again = signer.derive_bip85_mnemonic(0, 12).unwrap();
        assert_eq!(
            derived_mnemonic_12.to_string(),
            derived_mnemonic_0_again.to_string()
        );
    }
}
