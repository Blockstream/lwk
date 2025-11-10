use std::{ops::ControlFlow, sync::Arc};

use boltz_client::{
    boltz::{
        ChainSwapStates, GetReversePairsResponse, GetSubmarinePairsResponse, RevSwapStates,
        SubSwapStates, SwapRestoreResponse, Webhook,
    },
    network::Chain,
    Bolt11Invoice,
};
use elements::bitcoin;
use lwk_wollet::elements;

use crate::{
    prepare_pay_data::PreparePayDataSerializable, Error, InvoiceData, InvoiceDataSerializable,
    LightningPayment, PreparePayData, RescueFile, SwapStatus,
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

    pub fn swap_restore(&self) -> Result<Vec<SwapRestoreResponse>, Error> {
        let inner = self.runtime.block_on(self.inner.swap_restore())?;
        Ok(inner)
    }

    pub fn fetch_swaps_info(
        &self,
    ) -> Result<(GetReversePairsResponse, GetSubmarinePairsResponse), Error> {
        let inner = self.runtime.block_on(self.inner.fetch_swaps_info())?;
        Ok(inner)
    }
}

impl PreparePayResponse {
    pub fn complete_pay(self) -> Result<bool, Error> {
        let inner = self.runtime.block_on(self.inner.complete_pay())?;
        Ok(inner)
    }

    pub fn swap_id(&self) -> String {
        self.inner.swap_id()
    }

    pub fn uri(&self) -> String {
        self.inner.data.create_swap_response.bip21.clone()
    }

    pub fn uri_address(&self) -> String {
        self.inner.data.create_swap_response.address.clone()
    }

    pub fn uri_amount(&self) -> u64 {
        self.inner.data.create_swap_response.expected_amount
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

    pub fn swap_id(&self) -> String {
        self.inner.swap_id().to_string()
    }

    pub fn bolt11_invoice(&self) -> Bolt11Invoice {
        self.inner.bolt11_invoice()
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
    pub fn swap_id(&self) -> String {
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
