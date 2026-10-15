//! A transaction builder that borrows the wallet, for use from rust

use super::{BuiltTx, TxBuilder};
use crate::{
    issuance::{IssuanceRequest, ReissuanceRequest},
    liquidex::Validated,
    model::{ExternalUtxo, Recipient},
    pegin::PeginInput,
    Contract, Error, LiquidexProposal, UnvalidatedRecipient, Wollet,
};
use elements::{pset::PartiallySignedTransaction, Address, AssetId, OutPoint, Transaction};
use lwk_common::FeeRate;

/// A transaction builder.
#[derive(Debug)]
pub struct WolletTxBuilder<'a> {
    wollet: &'a Wollet,
    inner: TxBuilder,
}

impl<'a> WolletTxBuilder<'a> {
    /// Creates a transaction builder. Could be conveniently created with [`Wollet::tx_builder()`]
    pub fn new(wollet: &'a Wollet) -> Self {
        WolletTxBuilder {
            wollet,
            inner: TxBuilder::new(wollet.network()),
        }
    }

    /// Consume this builder and create a transaction
    pub fn finish(self) -> Result<PartiallySignedTransaction, Error> {
        self.inner.finish(self.wollet)
    }

    /// Consume this builder and create a transaction for AMP0
    #[cfg(feature = "amp0")]
    pub fn finish_for_amp0(self) -> Result<crate::amp0::Amp0Pset, Error> {
        self.inner.finish_for_amp0(self.wollet)
    }

    /// Build the transaction
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// Alternative to `finish()` that returns more than a PSET.
    pub fn build(self) -> Result<BuiltTx, Error> {
        self.inner.build(self.wollet)
    }

    /// Wrapper of [`TxBuilder::add_recipient()`]
    pub fn add_recipient(
        self,
        address: &Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_recipient(address, satoshi, asset_id)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_unvalidated_recipient()`]
    pub fn add_unvalidated_recipient(
        self,
        recipient: &UnvalidatedRecipient,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_unvalidated_recipient(recipient)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_validated_recipient()`]
    pub fn add_validated_recipient(self, recipient: Recipient) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.add_validated_recipient(recipient),
        }
    }

    /// Wrapper of [`TxBuilder::set_unvalidated_recipients()`]
    pub fn set_unvalidated_recipients(
        self,
        recipients: &[UnvalidatedRecipient],
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.set_unvalidated_recipients(recipients)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_lbtc_recipient()`]
    pub fn add_lbtc_recipient(self, address: &Address, satoshi: u64) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_lbtc_recipient(address, satoshi)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_burn()`]
    pub fn add_burn(self, satoshi: u64, asset_id: AssetId) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_burn(satoshi, asset_id)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_op_return()`]
    pub fn add_op_return(self, data: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_op_return(data)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_explicit_recipient()`]
    pub fn add_explicit_recipient(
        self,
        address: &Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self
                .inner
                .add_explicit_recipient(address, satoshi, asset_id)?,
        })
    }

    /// Wrapper of [`TxBuilder::fee_rate()`]
    pub fn fee_rate(self, fee_rate: Option<FeeRate>) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.fee_rate(fee_rate),
        }
    }

    /// Wrapper of [`TxBuilder::allow_high_fee_rate()`]
    pub fn allow_high_fee_rate(self) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.allow_high_fee_rate(),
        }
    }

    /// Wrapper of [`TxBuilder::enable_ct_discount()`]
    pub fn enable_ct_discount(self) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.enable_ct_discount(),
        }
    }

    /// Wrapper of [`TxBuilder::disable_ct_discount()`]
    pub fn disable_ct_discount(self) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.disable_ct_discount(),
        }
    }

    /// Wrapper of [`TxBuilder::add_input_rangeproofs()`]
    pub fn add_input_rangeproofs(self, add_rangeproofs: bool) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.add_input_rangeproofs(add_rangeproofs),
        }
    }

    /// Wrapper of [`TxBuilder::issue_asset()`]
    pub fn issue_asset(
        self,
        asset_sats: u64,
        asset_receiver: Option<Address>,
        token_sats: u64,
        token_receiver: Option<Address>,
        contract: Option<Contract>,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.issue_asset(
                asset_sats,
                asset_receiver,
                token_sats,
                token_receiver,
                contract,
            )?,
        })
    }

    /// Wrapper of [`TxBuilder::add_issuance()`]
    ///
    /// **Experimental**: this API might change without notice.
    pub fn add_issuance(self, request_builder: IssuanceRequest) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_issuance(request_builder)?,
        })
    }

    /// Wrapper of [`TxBuilder::reissue_asset()`]
    pub fn reissue_asset(
        self,
        asset_to_reissue: AssetId,
        satoshi_to_reissue: u64,
        asset_receiver: Option<Address>,
        issuance_tx: Option<Transaction>,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.reissue_asset(
                asset_to_reissue,
                satoshi_to_reissue,
                asset_receiver,
                issuance_tx,
            )?,
        })
    }

    /// Wrapper of [`TxBuilder::add_reissuance()`]
    ///
    /// **Experimental**: this API might change without notice.
    pub fn add_reissuance(self, reissuance_request: ReissuanceRequest) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_reissuance(reissuance_request)?,
        })
    }

    /// Wrapper of [`TxBuilder::drain_lbtc_wallet()`]
    pub fn drain_lbtc_wallet(self) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.drain_lbtc_wallet(),
        }
    }

    /// Wrapper of [`TxBuilder::drain_lbtc_to()`]
    pub fn drain_lbtc_to(self, address: &Address) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.drain_lbtc_to(address)?,
        })
    }

    /// Wrapper of [`TxBuilder::drain_lbtc_to_explicit()`]
    pub fn drain_lbtc_to_explicit(self, address: &Address) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.drain_lbtc_to_explicit(address)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_external_utxos()`]
    pub fn add_external_utxos(self, utxos: Vec<ExternalUtxo>) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_external_utxos(utxos)?,
        })
    }

    /// Wrapper of [`TxBuilder::add_pegin_input()`]
    pub fn add_pegin_input(self, input: PeginInput) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.add_pegin_input(input)?,
        })
    }

    /// Wrapper of [`TxBuilder::set_wallet_utxos()`]
    pub fn set_wallet_utxos(self, utxos: Vec<OutPoint>) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.set_wallet_utxos(utxos),
        }
    }

    /// Wrapper of [`TxBuilder::set_inputs_order()`]
    ///
    /// **Experimental**: this API might change without notice.
    pub fn set_inputs_order(self, inputs_order: Vec<OutPoint>) -> Self {
        Self {
            wollet: self.wollet,
            inner: self.inner.set_inputs_order(inputs_order),
        }
    }

    /// Wrapper of [`TxBuilder::liquidex_make()`]
    pub fn liquidex_make(
        self,
        utxo: OutPoint,
        address: &Address,
        satoshi: u64,
        asset_id: AssetId,
    ) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.liquidex_make(utxo, address, satoshi, asset_id)?,
        })
    }

    /// Wrapper of [`TxBuilder::liquidex_take()`]
    pub fn liquidex_take(self, proposals: Vec<LiquidexProposal<Validated>>) -> Result<Self, Error> {
        Ok(Self {
            wollet: self.wollet,
            inner: self.inner.liquidex_take(proposals)?,
        })
    }
}
