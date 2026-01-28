use std::{fmt, str::FromStr, sync::Arc};

use crate::{types::SecretKey, Chain, LwkError, Script};

/// The output descriptors, wrapper over [`lwk_wollet::WolletDescriptor`]
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct WolletDescriptor {
    inner: lwk_wollet::WolletDescriptor,
}

impl AsRef<lwk_wollet::WolletDescriptor> for WolletDescriptor {
    fn as_ref(&self) -> &lwk_wollet::WolletDescriptor {
        &self.inner
    }
}

impl From<lwk_wollet::WolletDescriptor> for WolletDescriptor {
    fn from(inner: lwk_wollet::WolletDescriptor) -> Self {
        Self { inner }
    }
}

impl From<&WolletDescriptor> for lwk_wollet::WolletDescriptor {
    fn from(desc: &WolletDescriptor) -> Self {
        desc.inner.clone()
    }
}

#[uniffi::export]
impl WolletDescriptor {
    /// Create a new descriptor from its string representation.
    #[uniffi::constructor]
    pub fn new(descriptor: &str) -> Result<Arc<Self>, LwkError> {
        let inner = lwk_wollet::WolletDescriptor::from_str(descriptor)?;
        Ok(Arc::new(WolletDescriptor { inner }))
    }

    /// Whether the descriptor is on the mainnet
    pub fn is_mainnet(&self) -> bool {
        self.inner.is_mainnet()
    }

    /// Derive the private blinding key
    pub fn derive_blinding_key(&self, script_pubkey: &Script) -> Option<Arc<SecretKey>> {
        self.inner
            .ct_descriptor()
            .map(|d| lwk_common::derive_blinding_key(d, &script_pubkey.into()))
            .ok()
            .flatten()
            .map(Into::into)
            .map(Arc::new)
    }

    /// Derive a scriptpubkey
    pub fn script_pubkey(&self, ext_int: Chain, index: u32) -> Result<Arc<Script>, LwkError> {
        self.inner
            .script_pubkey(ext_int.into(), index)
            .map_err(Into::into)
            .map(Into::into)
            .map(Arc::new)
    }

    /// Whether the descriptor is AMP0
    pub fn is_amp0(&self) -> bool {
        self.inner.is_amp0()
    }

    /// Return the descriptor encoded so that can be part of an URL
    pub fn url_encoded_descriptor(&self) -> Result<String, LwkError> {
        Ok(self.inner.url_encoded_descriptor()?)
    }
}

impl fmt::Display for WolletDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[cfg(test)]
mod tests {
    use lwk_wollet::ElementsNetwork;

    use crate::{Chain, Mnemonic, Signer, WolletDescriptor};
    use std::str::FromStr;

    #[test]
    fn wpkh_slip77_descriptor() {
        let mnemonic_str = lwk_test_util::TEST_MNEMONIC;
        let mnemonic = Mnemonic::new(mnemonic_str).unwrap();
        let network: crate::Network = ElementsNetwork::default_regtest().into();

        let signer = Signer::new(&mnemonic, &network).unwrap();
        let exp = "ct(slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023),elwpkh([73c5da0a/84'/1'/0']tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M/<0;1>/*))#2e4n992d";
        assert_eq!(signer.wpkh_slip77_descriptor().unwrap().to_string(), exp);

        let wollet_desc = lwk_wollet::WolletDescriptor::from_str(exp).unwrap();
        let desc: WolletDescriptor = wollet_desc.into();
        assert_eq!(desc.to_string(), exp);

        assert!(!desc.is_mainnet());

        assert_eq!(
            desc.script_pubkey(Chain::External, 0).unwrap().to_string(),
            "0014d0c4a3ef09e997b6e99e397e518fe3e41a118ca1"
        );

        assert_eq!(
            desc.script_pubkey(Chain::Internal, 0).unwrap().to_string(),
            "00142f34aa1cf00a53b055a291a03a7d45f0a6988b52"
        );
    }
}
