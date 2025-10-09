use std::{ops::ControlFlow, sync::Arc, time::Duration};

use boltz_client::Bolt11Invoice;
use lwk_wollet::{elements, ElementsNetwork};

use crate::{clients::ElectrumClient, Error, InvoiceData, PreparePayData, SwapStatus};

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
    ) -> Result<Self, Error> {
        let runtime = Arc::new(tokio::runtime::Runtime::new()?);
        let _guard = runtime.enter();
        let inner = super::LightningSession::new(network, client, timeout);
        Ok(Self { inner, runtime })
    }

    pub fn prepare_pay(
        &self,
        bolt11_invoice: &Bolt11Invoice,
        refund_address: &elements::Address,
    ) -> Result<PreparePayResponse, Error> {
        let inner = self
            .runtime
            .block_on(self.inner.prepare_pay(bolt11_invoice, refund_address))?;
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
    ) -> Result<InvoiceResponse, Error> {
        let inner =
            self.runtime
                .block_on(self.inner.invoice(amount, description, claim_address))?;
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
