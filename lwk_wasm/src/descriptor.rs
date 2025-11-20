use crate::Error;
use lwk_common::{multisig_desc, DescriptorBlindingKey, Multisig};
use std::str::FromStr;
use wasm_bindgen::prelude::*;

/// A wrapper that contains only the subset of CT descriptors handled by wollet
#[wasm_bindgen]
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

impl From<WolletDescriptor> for lwk_wollet::WolletDescriptor {
    fn from(desc: WolletDescriptor) -> Self {
        desc.inner
    }
}

#[wasm_bindgen]
impl WolletDescriptor {
    /// Creates a `WolletDescriptor`
    #[wasm_bindgen(constructor)]
    pub fn new(descriptor: &str) -> Result<WolletDescriptor, Error> {
        let desc = lwk_wollet::WolletDescriptor::from_str_relaxed(descriptor)?;
        Ok(desc.into())
    }

    /// Return the string representation of the descriptor, including the checksum.
    /// This representation can be used to recreate the descriptor via `new()`
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }

    /// Create a new multisig descriptor, where each participant is a keyorigin_xpub and it requires at least threshold signatures to spend.
    /// Errors if the threshold is 0 or greater than the number of participants.
    /// Uses slip77 for the blinding key.
    #[wasm_bindgen(js_name = newMultiWshSlip77)]
    pub fn new_multi_wsh_slip77(
        threshold: u32,
        participants: Vec<String>,
    ) -> Result<WolletDescriptor, Error> {
        let xpubs: Vec<_> = participants
            .iter()
            .map(|s| lwk_common::keyorigin_xpub_from_str(s))
            .collect::<Result<_, _>>()?;

        let desc = multisig_desc(
            threshold,
            xpubs,
            Multisig::Wsh,
            DescriptorBlindingKey::Slip77Rand,
        )
        .map_err(Error::Generic)?;
        let desc = lwk_wollet::WolletDescriptor::from_str(&desc)?;
        Ok(desc.into())
    }

    /// Whether the descriptor is for mainnet
    #[wasm_bindgen(js_name = isMainnet)]
    pub fn is_mainnet(&self) -> bool {
        self.inner.is_mainnet()
    }

    /// Whether the descriptor is AMP0
    #[wasm_bindgen(js_name = isAmp0)]
    pub fn is_amp0(&self) -> bool {
        self.inner.is_amp0()
    }
}

impl std::fmt::Display for WolletDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use wasm_bindgen_test::*;

    use crate::WolletDescriptor;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_descriptor() {
        let desc = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/<0;1>/*)))#efvhq75f";
        assert_eq!(desc, WolletDescriptor::new(desc).unwrap().to_string());

        // multiline
        let first = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/0/*)))";
        let second = "ct(slip77(0371e66dde8ab9f3cb19d2c20c8fa2d7bd1ddc73454e6b7ef15f0c5f624d4a86),elsh(wpkh([75ea4a43/49'/1776'/0']xpub6D3Y5EKNsmegjE7azkF2foAYFivHrV5u7tcnN2TXELxv1djNtabCHtp3jMvxqEhTU737mYSUqHD1sA5MdZXQ8DWJLNft1gwtpzXZDsRnrZd/1/*)))";
        let both = format!("{first}\n{second}");
        assert_eq!(desc, WolletDescriptor::new(&both).unwrap().to_string());

        assert!(WolletDescriptor::new(desc).unwrap().is_mainnet());
    }
}
