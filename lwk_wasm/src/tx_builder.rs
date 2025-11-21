use std::fmt::Display;

use lwk_wollet::{elements, UnvalidatedRecipient, Validated};
use wasm_bindgen::prelude::*;

use crate::{
    liquidex::ValidatedLiquidexProposal, Address, AssetId, Contract, Error, Network, OutPoint,
    Pset, Transaction, Wollet,
};

/// A transaction builder
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
    pub fn new(network: &Network) -> TxBuilder {
        TxBuilder {
            inner: lwk_wollet::TxBuilder::new(network.into()),
        }
    }

    /// Build the transaction
    pub fn finish(self, wollet: &Wollet) -> Result<Pset, Error> {
        Ok(self.inner.finish(wollet.as_ref())?.into())
    }

    /// Build the transaction for AMP0
    #[cfg(all(feature = "serial", target_arch = "wasm32"))]
    #[wasm_bindgen(js_name = finishForAmp0)]
    pub fn finish_for_amp0(self, wollet: &Wollet) -> Result<crate::amp0::Amp0Pset, Error> {
        Ok(self.inner.finish_for_amp0(wollet.as_ref())?.into())
    }

    /// Set the fee rate
    #[wasm_bindgen(js_name = feeRate)]
    pub fn fee_rate(self, fee_rate: Option<f32>) -> TxBuilder {
        self.inner.fee_rate(fee_rate).into()
    }

    /// Select all available L-BTC inputs
    #[wasm_bindgen(js_name = drainLbtcWallet)]
    pub fn drain_lbtc_wallet(self) -> TxBuilder {
        self.inner.drain_lbtc_wallet().into()
    }

    /// Sets the address to drain excess L-BTC to
    #[wasm_bindgen(js_name = drainLbtcTo)]
    pub fn drain_lbtc_to(self, address: Address) -> TxBuilder {
        self.inner.drain_lbtc_to(address.into()).into()
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

    /// Add explicit recipient
    #[wasm_bindgen(js_name = addExplicitRecipient)]
    pub fn add_explicit_recipient(
        self,
        address: Address,
        satoshi: u64,
        asset: &AssetId,
    ) -> Result<TxBuilder, Error> {
        Ok(self
            .inner
            .add_explicit_recipient(&address.into(), satoshi, (*asset).into())?
            .into())
    }

    /// Issue an asset
    ///
    /// There will be `asset_sats` units of this asset that will be received by
    /// `asset_receiver` if it's set, otherwise to an address of the wallet generating the issuance.
    ///
    /// There will be `token_sats` reissuance tokens that allow token holder to reissue the created
    /// asset. Reissuance token will be received by `token_receiver` if it's some, or to an
    /// address of the wallet generating the issuance if none.
    ///
    /// If a `contract` is provided, it's metadata will be committed in the generated asset id.
    ///
    /// Can't be used if `reissue_asset` has been called
    #[wasm_bindgen(js_name = issueAsset)]
    pub fn issue_asset(
        self,
        asset_sats: u64,
        asset_receiver: Option<Address>,
        token_sats: u64,
        token_receiver: Option<Address>,
        contract: Option<Contract>,
    ) -> Result<TxBuilder, Error> {
        Ok(self
            .inner
            .issue_asset(
                asset_sats,
                asset_receiver.map(Into::into),
                token_sats,
                token_receiver.map(Into::into),
                contract.map(Into::into),
            )?
            .into())
    }

    /// Reissue an asset
    ///
    /// reissue the asset defined by `asset_to_reissue`, provided the reissuance token is owned
    /// by the wallet generating te reissuance.
    ///
    /// Generated transaction will create `satoshi_to_reissue` new asset units, and they will be
    /// sent to the provided `asset_receiver` address if some, or to an address from the wallet
    /// generating the reissuance transaction if none.
    ///
    /// If the issuance transaction does not involve this wallet,
    /// pass the issuance transaction in `issuance_tx`.
    #[wasm_bindgen(js_name = reissueAsset)]
    pub fn reissue_asset(
        self,
        asset_to_reissue: AssetId,
        satoshi_to_reissue: u64,
        asset_receiver: Option<Address>,
        issuance_tx: Option<Transaction>,
    ) -> Result<TxBuilder, Error> {
        Ok(self
            .inner
            .reissue_asset(
                asset_to_reissue.into(),
                satoshi_to_reissue,
                asset_receiver.map(Into::into),
                issuance_tx.map(Into::into),
            )?
            .into())
    }

    /// Switch to manual coin selection by giving a list of internal UTXOs to use.
    ///
    /// All passed UTXOs are added to the transaction.
    /// No other wallet UTXO is added to the transaction, caller is supposed to add enough UTXOs to
    /// cover for all recipients and fees.
    ///
    /// This method never fails, any error will be raised in [`TxBuilder::finish`].
    ///
    /// Possible errors:
    /// * OutPoint doesn't belong to the wallet
    /// * Insufficient funds (remember to include L-BTC utxos for fees)
    #[wasm_bindgen(js_name = setWalletUtxos)]
    pub fn set_wallet_utxos(self, outpoints: Vec<OutPoint>) -> TxBuilder {
        let outpoints: Vec<elements::OutPoint> = outpoints.into_iter().map(Into::into).collect();
        self.inner.set_wallet_utxos(outpoints).into()
    }

    /// Return a string representation of the transaction builder (mostly for debugging)
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        self.to_string()
    }

    /// Set data to create a PSET from which you
    /// can create a LiquiDEX proposal
    #[wasm_bindgen(js_name = liquidexMake)]
    pub fn liquidex_make(
        self,
        utxo: OutPoint,
        address: Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<TxBuilder, Error> {
        Ok(self
            .inner
            .liquidex_make(utxo.into(), &(address.into()), satoshi, asset_id.into())?
            .into())
    }

    /// Set data to take LiquiDEX proposals
    #[wasm_bindgen(js_name = liquidexTake)]
    pub fn liquidex_take(self, proposals: Vec<ValidatedLiquidexProposal>) -> Result<Self, Error> {
        let proposals: Vec<lwk_wollet::LiquidexProposal<Validated>> =
            proposals.into_iter().map(Into::into).collect();
        Ok(self.inner.liquidex_take(proposals)?.into())
    }

    /// Add input rangeproofs
    #[wasm_bindgen(js_name = addInputRangeproofs)]
    pub fn add_input_rangeproofs(self, add_rangeproofs: bool) -> TxBuilder {
        self.inner.add_input_rangeproofs(add_rangeproofs).into()
    }
}

impl Display for TxBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::*;

    use crate::{Network, OutPoint};

    use super::TxBuilder;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_builder() {
        let network = Network::mainnet();
        let policy = network.policy_asset();

        let mut builder = TxBuilder::new(&network);
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, recipients: [], fee_rate: 100.0, ct_discount: true, issuance_request: None, drain_lbtc: false, drain_to: None, external_utxos: [], selected_utxos: None, add_input_rangeproofs: true, is_liquidex_make: false, liquidex_proposals: [] }");

        builder = builder.fee_rate(Some(200.0));
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, recipients: [], fee_rate: 200.0, ct_discount: true, issuance_request: None, drain_lbtc: false, drain_to: None, external_utxos: [], selected_utxos: None, add_input_rangeproofs: true, is_liquidex_make: false, liquidex_proposals: [] }");

        builder = builder.add_burn(1000, &policy);
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, recipients: [Recipient { satoshi: 1000, script_pubkey: Script(OP_RETURN), blinding_pubkey: None, asset: 6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d }], fee_rate: 200.0, ct_discount: true, issuance_request: None, drain_lbtc: false, drain_to: None, external_utxos: [], selected_utxos: None, add_input_rangeproofs: true, is_liquidex_make: false, liquidex_proposals: [] }");

        let o = OutPoint::new(
            "[elements]b93dbfb3fa1929b6f82ed46c4a5d8e1c96239ca8b3d9fce00c321d7dadbdf6e0:0",
        )
        .unwrap();
        builder = builder.set_wallet_utxos(vec![o]);
        assert_eq!(builder.to_string(), "TxBuilder { network: Liquid, recipients: [Recipient { satoshi: 1000, script_pubkey: Script(OP_RETURN), blinding_pubkey: None, asset: 6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d }], fee_rate: 200.0, ct_discount: true, issuance_request: None, drain_lbtc: false, drain_to: None, external_utxos: [], selected_utxos: Some([OutPoint { txid: b93dbfb3fa1929b6f82ed46c4a5d8e1c96239ca8b3d9fce00c321d7dadbdf6e0, vout: 0 }]), add_input_rangeproofs: true, is_liquidex_make: false, liquidex_proposals: [] }");
    }
}
