use std::{fmt::Display, sync::Mutex};

use lwk_wollet::UnvalidatedAddressee;

use crate::{types::AssetId, Address, LwkError, Network, Pset, Wollet};

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
    "This transaction builder already called finish".into()
}

#[uniffi::export]
impl TxBuilder {
    /// Construct a transaction builder
    #[uniffi::constructor]
    pub fn new(network: &Network) -> Self {
        TxBuilder {
            inner: Mutex::new(Some(lwk_wollet::TxBuilder::new((*network).into()))),
            network: (*network).into(),
        }
    }

    /// Build the transaction
    pub fn finish(&self, wollet: &Wollet) -> Result<Pset, LwkError> {
        let mut lock = self.inner.lock()?;
        let wollet = wollet.inner_wollet()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        Ok(inner.finish(&wollet)?.into())
    }

    /// Set the fee rate
    pub fn fee_rate(&self, rate: Option<f32>) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock = Some(inner.fee_rate(rate));
        Ok(())
    }

    /// Add a recipient receiving L-BTC
    pub fn add_lbtc_recipient(&self, address: &Address, satoshi: u64) -> Result<(), LwkError> {
        let unvalidated_recipient = UnvalidatedAddressee::lbtc(address.to_string(), satoshi);
        let recipient = unvalidated_recipient.validate(&self.network)?;
        self.add_validated_recipient(recipient)
    }

    /// Add a recipient receiving the given asset
    pub fn add_recipient(
        &self,
        address: &Address,
        satoshi: u64,
        asset: &AssetId,
    ) -> Result<(), LwkError> {
        let unvalidated_recipient = UnvalidatedAddressee {
            satoshi,
            address: address.to_string(),
            asset: asset.to_string(),
        };
        let recipient = unvalidated_recipient.validate(&self.network)?;
        self.add_validated_recipient(recipient)
    }

    /// Burn satoshi units of the given asset
    pub fn add_burn(&self, satoshi: u64, asset: &AssetId) -> Result<(), LwkError> {
        let unvalidated_recipient = UnvalidatedAddressee::burn(asset.to_string(), satoshi);
        let recipient = unvalidated_recipient.validate(&self.network)?;
        self.add_validated_recipient(recipient)
    }
}

impl TxBuilder {
    fn add_validated_recipient(&self, recipient: lwk_wollet::Addressee) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let inner = lock.take().ok_or_else(builder_finished)?;
        *lock = Some(inner.add_validated_recipient(recipient));
        Ok(())
    }
}
