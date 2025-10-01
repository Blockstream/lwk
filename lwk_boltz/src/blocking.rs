use std::sync::Arc;

use crate::Error;

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
    pub fn new(inner: super::LightningSession) -> Result<Self, Error> {
        Ok(Self {
            inner,
            runtime: Arc::new(tokio::runtime::Runtime::new()?),
        })
    }

    pub fn prepare_pay(&self, bolt11_invoice: &str) -> Result<PreparePayResponse, Error> {
        let inner = self
            .runtime
            .block_on(self.inner.prepare_pay(bolt11_invoice))?;
        Ok(PreparePayResponse {
            inner,
            runtime: self.runtime.clone(),
        })
    }

    pub fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: String,
    ) -> Result<InvoiceResponse, Error> {
        let inner =
            self.runtime
                .block_on(self.inner.invoice(amount, description, claim_address))?;
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
}

impl InvoiceResponse {
    pub fn complete_pay(self) -> Result<bool, Error> {
        let inner = self.runtime.block_on(self.inner.complete_pay())?;
        Ok(inner)
    }
}
