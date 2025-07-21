use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};

use lwk_wollet::UnvalidatedRecipient;

use crate::{
    types::AssetId, Address, Contract, ExternalUtxo, LwkError, Network, OutPoint, Pset,
    Transaction, ValidatedLiquidexProposal, Wollet,
};

/// Wrapper over [`lwk_wollet::TxBuilder`]
#[derive(uniffi::Object, Debug)]
#[uniffi::export(Display)]
pub struct TxBuilder {
    /// Uniffi doesn't allow to accept self and consume the parameter (everything is behind Arc)
    /// So, inside the Mutex we have an option that allow to consume the inner builder and also
    /// to emulate the consumption of this builder after the call to finish.
    inner: Mutex<Option<lwk_wollet::TxBuilder>>,

    /// We are keeping a copy of the network here so that we can read it without lockig the mutex
    network: lwk_wollet::ElementsNetwork,
}

impl Display for TxBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.inner.lock() {
            Ok(r) => write!(f, "{:?}", r.as_ref()),
            Err(_) => write!(f, "{:?}", self.inner),
        }
    }
}

fn builder_finished() -> LwkError {
    "This transaction builder already called finish or errored".into()
}

#[uniffi::export]
impl TxBuilder {
    /// Construct a transaction builder
    #[uniffi::constructor]
    pub fn new(network: &Network) -> Self {
        TxBuilder {
            inner: Mutex::new(Some(lwk_wollet::TxBuilder::new(network.into()))),
            network: network.into(),
        }
    }

    /// Build the transaction
    pub fn finish(&self, wollet: &Wollet) -> Result<Pset, LwkError> {
        let mut lock = self.inner.lock()?;
        let wollet = wollet.inner_wollet()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        Ok(inner.finish(&wollet)?.into())
    }

    /// Fee rate in sats/kvb
    /// Multiply sats/vb value by 1000 i.e. 1.0 sat/byte = 1000.0 sat/kvb
    pub fn fee_rate(&self, rate: Option<f32>) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock = Some(inner.fee_rate(rate));
        Ok(())
    }

    /// Select all available L-BTC inputs
    pub fn drain_lbtc_wallet(&self) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock = Some(inner.drain_lbtc_wallet());
        Ok(())
    }

    /// Sets the address to drain excess L-BTC to
    pub fn drain_lbtc_to(&self, address: &Address) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock = Some(inner.drain_lbtc_to(address.into()));
        Ok(())
    }

    /// Add a recipient receiving L-BTC
    pub fn add_lbtc_recipient(&self, address: &Address, satoshi: u64) -> Result<(), LwkError> {
        let unvalidated_recipient = UnvalidatedRecipient::lbtc(address.to_string(), satoshi);
        let recipient = unvalidated_recipient.validate(self.network)?;
        self.add_validated_recipient(recipient)
    }

    /// Add a recipient receiving the given asset
    pub fn add_recipient(
        &self,
        address: &Address,
        satoshi: u64,
        asset: &AssetId,
    ) -> Result<(), LwkError> {
        let unvalidated_recipient = UnvalidatedRecipient {
            satoshi,
            address: address.to_string(),
            asset: asset.to_string(),
        };
        let recipient = unvalidated_recipient.validate(self.network)?;
        self.add_validated_recipient(recipient)
    }

    /// Burn satoshi units of the given asset
    pub fn add_burn(&self, satoshi: u64, asset: &AssetId) -> Result<(), LwkError> {
        let unvalidated_recipient = UnvalidatedRecipient::burn(asset.to_string(), satoshi);
        let recipient = unvalidated_recipient.validate(self.network)?;
        self.add_validated_recipient(recipient)
    }

    /// Add explicit recipient
    pub fn add_explicit_recipient(
        &self,
        address: &Address,
        satoshi: u64,
        asset: &AssetId,
    ) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock = Some(inner.add_explicit_recipient(&(address.into()), satoshi, (*asset).into())?);
        Ok(())
    }

    /// Issue an asset, wrapper of [`lwk_wollet::TxBuilder::issue_asset()`]
    pub fn issue_asset(
        &self,
        asset_sats: u64,
        asset_receiver: Option<Arc<Address>>,
        token_sats: u64,
        token_receiver: Option<Arc<Address>>,
        contract: Option<Arc<Contract>>,
    ) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        let new_inner = inner.issue_asset(
            asset_sats,
            asset_receiver.map(|e| e.as_ref().into()),
            token_sats,
            token_receiver.map(|e| e.as_ref().into()),
            contract.map(|e| e.as_ref().into()),
        )?;
        *lock = Some(new_inner);
        Ok(())
    }

    /// Reissue an asset, wrapper of [`lwk_wollet::TxBuilder::reissue_asset()`]
    pub fn reissue_asset(
        &self,
        asset_to_reissue: AssetId,
        satoshi_to_reissue: u64,
        asset_receiver: Option<Arc<Address>>,
        issuance_tx: Option<Arc<Transaction>>,
    ) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        let new_inner = inner.reissue_asset(
            asset_to_reissue.into(),
            satoshi_to_reissue,
            asset_receiver.map(|e| e.as_ref().into()),
            issuance_tx.map(|e| e.as_ref().into()),
        )?;
        *lock = Some(new_inner);
        Ok(())
    }

    /// Manual coin selection, wrapper of [`lwk_wollet::TxBuilder::set_wallet_utxos()`]
    pub fn set_wallet_utxos(&self, utxos: Vec<Arc<OutPoint>>) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        let utxos = utxos
            .into_iter()
            .map(|arc| elements::OutPoint::from(arc.as_ref()))
            .collect();
        *lock = Some(inner.set_wallet_utxos(utxos));
        Ok(())
    }

    /// Add external utxos, wrapper of [`lwk_wollet::TxBuilder::add_external_utxos()`]
    pub fn add_external_utxos(&self, utxos: Vec<Arc<ExternalUtxo>>) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        let utxos = utxos
            .into_iter()
            .map(|arc| lwk_wollet::ExternalUtxo::from(arc.as_ref()))
            .collect();
        *lock = Some(inner.add_external_utxos(utxos)?);
        Ok(())
    }

    pub fn liquidex_make(
        &self,
        utxo: &OutPoint,
        address: &Address,
        amount: u64,
        asset: AssetId,
    ) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock = Some(inner.liquidex_make(utxo.into(), address.as_ref(), amount, asset.into())?);
        Ok(())
    }

    pub fn liquidex_take(
        &self,
        proposals: Vec<Arc<ValidatedLiquidexProposal>>,
    ) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock =
            Some(inner.liquidex_take(proposals.into_iter().map(|p| p.as_ref().into()).collect())?);
        Ok(())
    }
}

impl TxBuilder {
    fn add_validated_recipient(&self, recipient: lwk_wollet::Recipient) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock = Some(inner.add_validated_recipient(recipient));
        Ok(())
    }
}
