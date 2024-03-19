use std::fmt::Display;

use lwk_wollet::UnvalidatedRecipient;
use wasm_bindgen::prelude::*;

use crate::{Address, AssetId, Error, Network, Pset, Wollet};

#[wasm_bindgen]
#[derive(Debug)]
pub struct TxBuilder {
    inner: lwk_wollet::TxBuilder,
}

impl From<lwk_wollet::TxBuilder> for TxBuilder {
    fn from(value: lwk_wollet::TxBuilder) -> Self {
        Self { inner: value }
    }
}

impl From<TxBuilder> for lwk_wollet::TxBuilder {
    fn from(value: TxBuilder) -> Self {
        value.inner
    }
}

#[wasm_bindgen]
impl TxBuilder {
    /// Creates a transaction builder
    #[wasm_bindgen(constructor)]
    pub fn new(network: Network) -> TxBuilder {
        TxBuilder {
            inner: lwk_wollet::TxBuilder::new(network.into()),
        }
    }

    /// Build the transaction
    pub fn finish(self, wollet: &Wollet) -> Result<Pset, Error> {
        Ok(self.inner.finish(wollet.as_ref())?.into())
    }

    /// Set the fee rate
    #[wasm_bindgen(js_name = feeRate)]
    pub fn fee_rate(self, fee_rate: Option<f32>) -> TxBuilder {
        self.inner.fee_rate(fee_rate).into()
    }

    /// Add a recipient receiving L-BTC
    ///
    /// Errors if address's network is incompatible
    #[wasm_bindgen(js_name = addLbtcRecipient)]
    pub fn add_lbtc_recipient(self, address: &Address, satoshi: u64) -> Result<TxBuilder, Error> {
        let unvalidated_recipient = UnvalidatedRecipient::lbtc(address.to_string(), satoshi);
        // TODO error variant should contain the TxBuilder so that caller can recover it
        Ok(self
            .inner
            .add_unvalidated_recipient(&unvalidated_recipient)?
            .into())
    }

    /// Add a recipient receiving the given asset
    ///
    /// Errors if address's network is incompatible
    #[wasm_bindgen(js_name = addRecipient)]
    pub fn add_recipient(
        self,
        address: &Address,
        satoshi: u64,
        asset: &AssetId,
    ) -> Result<TxBuilder, Error> {
        let unvalidated_recipient = UnvalidatedRecipient {
            satoshi,
            address: address.to_string(),
            asset: asset.to_string(),
        };
        Ok(self
            .inner
            .add_unvalidated_recipient(&unvalidated_recipient)?
            .into())
    }

    /// Burn satoshi units of the given asset
    #[wasm_bindgen(js_name = addBurn)]
    pub fn add_burn(self, satoshi: u64, asset: &AssetId) -> TxBuilder {
        let unvalidated_recipient = UnvalidatedRecipient::burn(asset.to_string(), satoshi);
        self.inner
            .add_unvalidated_recipient(&unvalidated_recipient)
            .expect("recipient can't be invalid")
            .into()
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }
}

impl Display for TxBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use crate::Network;

    use super::TxBuilder;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_builder() {
        let network = Network::mainnet();
        let policy = network.policy_asset();

        let mut builder = TxBuilder::new(network);
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, addressees: [], fee_rate: 100.0, issuance_request: None }");

        builder = builder.fee_rate(Some(200.0));
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, addressees: [], fee_rate: 200.0, issuance_request: None }");

        builder = builder.add_burn(1000, &policy);
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, addressees: [Recipient { satoshi: 1000, script_pubkey: Script(OP_RETURN OP_0), blinding_pubkey: None, asset: 6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d }], fee_rate: 200.0, issuance_request: None }");
    }
}
