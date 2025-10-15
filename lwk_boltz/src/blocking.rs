use std::{ops::ControlFlow, sync::Arc, time::Duration};

use bip39::Mnemonic;
use boltz_client::{
    boltz::{RevSwapStates, SubSwapStates, Webhook},
    Bolt11Invoice,
};
use lwk_wollet::{elements, ElementsNetwork};

use crate::{clients::ElectrumClient, Error, InvoiceData, PreparePayData, RescueFile, SwapStatus};

pub struct LightningSession {
    inner: super::LightningSession,
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

impl LightningSession {
    pub fn new(
        network: ElementsNetwork,
        client: Arc<ElectrumClient>,
        timeout: Option<Duration>,
        mnemonic: Option<Mnemonic>,
    ) -> Result<Self, Error> {
        let runtime = Arc::new(tokio::runtime::Runtime::new()?);
        let _guard = runtime.enter();
        let inner = runtime.block_on(super::LightningSession::new(
            network, client, timeout, mnemonic,
        ))?;
        Ok(Self { inner, runtime })
    }

    pub fn prepare_pay(
        &self,
        bolt11_invoice: &Bolt11Invoice,
        refund_address: &elements::Address,
        webhook: Option<Webhook<SubSwapStates>>,
    ) -> Result<PreparePayResponse, Error> {
        let inner = self.runtime.block_on(self.inner.prepare_pay(
            bolt11_invoice,
            refund_address,
            webhook,
        ))?;
        Ok(PreparePayResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn restore_prepare_pay(&self, data: &str) -> Result<PreparePayResponse, Error> {
        let data = PreparePayData::deserialize(data)?;
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

    pub fn restore_invoice(&self, data: &str) -> Result<InvoiceResponse, Error> {
        let data = InvoiceData::deserialize(data)?;
        let inner = self.runtime.block_on(self.inner.restore_invoice(data))?;
        Ok(InvoiceResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn rescue_file(&self) -> RescueFile {
        self.inner.rescue_file()
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
