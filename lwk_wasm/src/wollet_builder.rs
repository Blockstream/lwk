use wasm_bindgen::prelude::*;

use crate::{Error, Network, Wollet, WolletDescriptor};

/// A builder for constructing [`Wollet`] instances.
#[wasm_bindgen]
pub struct WolletBuilder {
    inner: lwk_wollet::WolletBuilder,
}

impl From<lwk_wollet::WolletBuilder> for WolletBuilder {
    fn from(value: lwk_wollet::WolletBuilder) -> Self {
        Self { inner: value }
    }
}

#[wasm_bindgen]
impl WolletBuilder {
    /// Create a builder for a watch-only wallet.
    #[wasm_bindgen(constructor)]
    pub fn new(network: &Network, descriptor: &WolletDescriptor) -> Self {
        lwk_wollet::WolletBuilder::new(network.into(), descriptor.into()).into()
    }

    /// Set the threshold used to merge persisted updates during build.
    ///
    /// `None` disables merging (default behavior).
    #[wasm_bindgen(js_name = withMergeThreshold)]
    pub fn with_merge_threshold(self, merge_threshold: Option<u32>) -> Self {
        self.inner
            .with_merge_threshold(merge_threshold.map(|t| t as usize))
            .into()
    }

    /// Experimental: set the wallet as "utxo only".
    #[wasm_bindgen(js_name = utxoOnly)]
    pub fn utxo_only(self, utxo_only: bool) -> Self {
        self.inner.utxo_only(utxo_only).into()
    }

    /// Build the wallet from this builder.
    pub fn build(self) -> Result<Wollet, Error> {
        Ok(self.inner.build()?.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let mnemonic = crate::Mnemonic::new(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .unwrap();
        let network = Network::regtest_default();
        let signer = crate::Signer::new(&mnemonic, &network).unwrap();
        let descriptor = signer.wpkh_slip77_descriptor().unwrap();

        let wollet = WolletBuilder::new(&network, &descriptor)
            .with_merge_threshold(Some(2))
            .utxo_only(true)
            .build()
            .unwrap();

        assert_eq!(wollet.address(Some(0)).unwrap().index(), 0);
    }
}
