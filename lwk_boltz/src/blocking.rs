use std::{ops::ControlFlow, sync::Arc};

use boltz_client::{
    boltz::{
        ChainSwapStates, GetChainPairsResponse, GetReversePairsResponse, GetSubmarinePairsResponse,
        RevSwapStates, SubSwapStates, SwapRestoreResponse, Webhook,
    },
    network::Chain,
    Bolt11Invoice,
};
use elements::bitcoin;
use lwk_wollet::elements;

use crate::{
    prepare_pay_data::PreparePayDataSerializable, ChainSwapData, ChainSwapDataSerializable, Error,
    InvoiceData, InvoiceDataSerializable, LightningPayment, PreparePayData, QuoteBuilder,
    RescueFile, SwapPersistence, SwapStatus,
};

pub struct BoltzSession {
    inner: super::BoltzSession,
    runtime: Arc<tokio::runtime::Runtime>,
}

pub struct PreparePayResponse {
    inner: super::PreparePayResponse,
    runtime: Arc<tokio::runtime::Runtime>,
}

pub struct InvoiceResponse {
    inner: super::InvoiceResponse,
    runtime: Arc<tokio::runtime::Runtime>,
}

pub struct LockupResponse {
    inner: super::LockupResponse,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl BoltzSession {
    /// Internal method to construct a blocking session from an async session and runtime
    pub(crate) fn new_from_async(
        inner: super::BoltzSession,
        runtime: Arc<tokio::runtime::Runtime>,
    ) -> Self {
        Self { inner, runtime }
    }

    pub fn prepare_pay(
        &self,
        lightning_payment: &LightningPayment,
        refund_address: &elements::Address,
        webhook: Option<Webhook<SubSwapStates>>,
    ) -> Result<PreparePayResponse, Error> {
        let inner = self.runtime.block_on(self.inner.prepare_pay(
            lightning_payment,
            refund_address,
            webhook,
        ))?;
        Ok(PreparePayResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn restore_prepare_pay(
        &self,
        data: PreparePayDataSerializable,
    ) -> Result<PreparePayResponse, Error> {
        let inner = self
            .runtime
            .block_on(self.inner.restore_prepare_pay(data))?;
        Ok(PreparePayResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: &elements::Address,
        webhook: Option<Webhook<RevSwapStates>>,
    ) -> Result<InvoiceResponse, Error> {
        let inner = self.runtime.block_on(self.inner.invoice(
            amount,
            description,
            claim_address,
            webhook,
        ))?;
        Ok(InvoiceResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn restore_invoice(&self, data: InvoiceDataSerializable) -> Result<InvoiceResponse, Error> {
        let inner = self.runtime.block_on(self.inner.restore_invoice(data))?;
        Ok(InvoiceResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn btc_to_lbtc(
        &self,
        amount: u64,
        refund_address: &bitcoin::Address,
        claim_address: &elements::Address,
        webhook: Option<Webhook<ChainSwapStates>>,
    ) -> Result<LockupResponse, Error> {
        let inner = self.runtime.block_on(self.inner.btc_to_lbtc(
            amount,
            refund_address,
            claim_address,
            webhook,
        ))?;
        Ok(LockupResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn lbtc_to_btc(
        &self,
        amount: u64,
        refund_address: &elements::Address,
        claim_address: &bitcoin::Address,
        webhook: Option<Webhook<ChainSwapStates>>,
    ) -> Result<LockupResponse, Error> {
        let inner = self.runtime.block_on(self.inner.lbtc_to_btc(
            amount,
            refund_address,
            claim_address,
            webhook,
        ))?;
        Ok(LockupResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn restore_lockup(&self, data: ChainSwapDataSerializable) -> Result<LockupResponse, Error> {
        let inner = self.runtime.block_on(self.inner.restore_lockup(data))?;
        Ok(LockupResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn rescue_file(&self) -> RescueFile {
        self.inner.rescue_file()
    }

    pub fn restorable_reverse_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        claim_address: &elements::Address,
    ) -> Result<Vec<InvoiceData>, Error> {
        let inner = self
            .runtime
            .block_on(self.inner.restorable_reverse_swaps(swaps, claim_address))?;
        Ok(inner)
    }

    pub fn restorable_submarine_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        refund_address: &elements::Address,
    ) -> Result<Vec<PreparePayData>, Error> {
        let inner = self
            .runtime
            .block_on(self.inner.restorable_submarine_swaps(swaps, refund_address))?;
        Ok(inner)
    }

    pub fn restorable_btc_to_lbtc_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        claim_address: &elements::Address,
        refund_address: &bitcoin::Address,
    ) -> Result<Vec<ChainSwapData>, Error> {
        let inner = self
            .runtime
            .block_on(self.inner.restorable_btc_to_lbtc_swaps(
                swaps,
                claim_address,
                refund_address,
            ))?;
        Ok(inner)
    }

    pub fn restorable_lbtc_to_btc_swaps(
        &self,
        swaps: &[SwapRestoreResponse],
        claim_address: &bitcoin::Address,
        refund_address: &elements::Address,
    ) -> Result<Vec<ChainSwapData>, Error> {
        let inner = self
            .runtime
            .block_on(self.inner.restorable_lbtc_to_btc_swaps(
                swaps,
                claim_address,
                refund_address,
            ))?;
        Ok(inner)
    }

    pub fn swap_restore(&self) -> Result<Vec<SwapRestoreResponse>, Error> {
        let inner = self.runtime.block_on(self.inner.swap_restore())?;
        Ok(inner)
    }

    /// Get the list of pending swap IDs from the store
    ///
    /// See [`crate::BoltzSession::pending_swap_ids()`]
    pub fn pending_swap_ids(&self) -> Result<Vec<String>, Error> {
        self.inner.pending_swap_ids()
    }

    /// Get the list of completed swap IDs from the store
    ///
    /// See [`crate::BoltzSession::completed_swap_ids()`]
    pub fn completed_swap_ids(&self) -> Result<Vec<String>, Error> {
        self.inner.completed_swap_ids()
    }

    /// Get the raw swap data for a specific swap ID from the store
    ///
    /// See [`crate::BoltzSession::get_swap_data()`]
    pub fn get_swap_data(&self, swap_id: &str) -> Result<Option<String>, Error> {
        self.inner.get_swap_data(swap_id)
    }

    /// Remove a swap from the store
    ///
    /// See [`crate::BoltzSession::remove_swap()`]
    pub fn remove_swap(&self, swap_id: &str) -> Result<(), Error> {
        self.inner.remove_swap(swap_id)
    }

    pub fn next_index_to_use(&self) -> u32 {
        self.inner.next_index_to_use()
    }

    pub fn set_next_index_to_use(&self, next_index_to_use: u32) {
        self.inner.set_next_index_to_use(next_index_to_use);
    }

    pub fn fetch_swaps_info(
        &self,
    ) -> Result<
        (
            GetReversePairsResponse,
            GetSubmarinePairsResponse,
            GetChainPairsResponse,
        ),
        Error,
    > {
        let swap_info = self.runtime.block_on(self.inner.fetch_swaps_info())?;
        Ok((
            swap_info.reverse_pairs,
            swap_info.submarine_pairs,
            swap_info.chain_pairs,
        ))
    }

    /// Refresh the cached pairs data from the Boltz API
    ///
    /// This updates the internal cache used by [`BoltzSession::quote()`].
    pub fn refresh_swap_info(&self) -> Result<(), Error> {
        self.runtime.block_on(self.inner.refresh_swap_info())
    }

    /// Create a quote builder for calculating swap fees
    ///
    /// This uses the cached pairs data from session initialization.
    pub fn quote(&self, send_amount: u64) -> QuoteBuilder {
        self.runtime.block_on(self.inner.quote(send_amount))
    }

    /// Create a quote builder for calculating send amount from desired receive amount
    ///
    /// This is the inverse of [`BoltzSession::quote()`].
    pub fn quote_receive(&self, receive_amount: u64) -> QuoteBuilder {
        self.runtime
            .block_on(self.inner.quote_receive(receive_amount))
    }
}

impl PreparePayResponse {
    pub fn complete_pay(self) -> Result<bool, Error> {
        let inner = self.runtime.block_on(self.inner.complete_pay())?;
        Ok(inner)
    }

    pub fn swap_id(&self) -> &str {
        self.inner.swap_id()
    }

    pub fn uri(&self) -> String {
        self.inner.uri()
    }

    pub fn uri_address(&self) -> Result<elements::Address, Error> {
        self.inner.uri_address()
    }

    pub fn uri_amount(&self) -> u64 {
        self.inner.uri_amount()
    }

    /// See [`crate::PreparePayResponse::fee()`]
    pub fn fee(&self) -> Option<u64> {
        self.inner.fee()
    }

    /// See [`crate::PreparePayResponse::boltz_fee()`]
    pub fn boltz_fee(&self) -> Option<u64> {
        self.inner.boltz_fee()
    }

    pub fn serialize(&self) -> Result<String, Error> {
        self.inner.serialize()
    }

    pub fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        let inner = self.runtime.block_on(self.inner.advance())?;
        Ok(inner)
    }
}

impl InvoiceResponse {
    pub fn complete_pay(self) -> Result<bool, Error> {
        let inner = self.runtime.block_on(self.inner.complete_pay())?;
        Ok(inner)
    }

    pub fn swap_id(&self) -> &str {
        self.inner.swap_id()
    }

    pub fn bolt11_invoice(&self) -> Bolt11Invoice {
        self.inner.bolt11_invoice()
    }

    /// See [`crate::InvoiceResponse::fee()`]
    pub fn fee(&self) -> Option<u64> {
        self.inner.fee()
    }

    /// See [`crate::InvoiceResponse::boltz_fee()`]
    pub fn boltz_fee(&self) -> Option<u64> {
        self.inner.boltz_fee()
    }

    /// See [`crate::InvoiceResponse::claim_txid()`]
    pub fn claim_txid(&self) -> Option<&str> {
        self.inner.claim_txid()
    }

    pub fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        let inner = self.runtime.block_on(self.inner.advance())?;
        Ok(inner)
    }

    pub fn serialize(&self) -> Result<String, Error> {
        self.inner.serialize()
    }
}

impl LockupResponse {
    pub fn swap_id(&self) -> &str {
        self.inner.swap_id()
    }

    pub fn lockup_address(&self) -> &str {
        self.inner.lockup_address()
    }

    pub fn expected_amount(&self) -> u64 {
        self.inner.expected_amount()
    }

    pub fn chain_from(&self) -> Chain {
        self.inner.chain_from()
    }

    pub fn chain_to(&self) -> Chain {
        self.inner.chain_to()
    }

    /// See [`crate::LockupResponse::fee()`]
    pub fn fee(&self) -> Option<u64> {
        self.inner.fee()
    }

    /// See [`crate::LockupResponse::boltz_fee()`]
    pub fn boltz_fee(&self) -> Option<u64> {
        self.inner.boltz_fee()
    }

    pub fn advance(&mut self) -> Result<ControlFlow<bool, SwapStatus>, Error> {
        let inner = self.runtime.block_on(self.inner.advance())?;
        Ok(inner)
    }

    pub fn serialize(&self) -> Result<String, Error> {
        self.inner.serialize()
    }

    pub fn complete(self) -> Result<bool, Error> {
        let inner = self.runtime.block_on(self.inner.complete())?;
        Ok(inner)
    }
}
