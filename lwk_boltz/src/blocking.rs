use std::sync::Arc;

use lwk_wollet::ElementsNetwork;

use crate::{clients::ElectrumClient, Error};

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
    pub fn new(network: ElementsNetwork, client: ElectrumClient) -> Result<Self, Error> {
        let runtime = Arc::new(tokio::runtime::Runtime::new()?);
        let _guard = runtime.enter();
        let inner = super::LightningSession::new(network, client);
        Ok(Self { inner, runtime })
    }

    pub fn prepare_pay(
        &self,
        bolt11_invoice: &str,
        refund_address: &str,
    ) -> Result<PreparePayResponse, Error> {
        let inner = self
            .runtime
            .block_on(self.inner.prepare_pay(bolt11_invoice, refund_address))?;
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

    pub fn uri(&self) -> String {
        self.inner.uri.clone()
    }
}

impl InvoiceResponse {
    pub fn complete_pay(self) -> Result<bool, Error> {
        let inner = self.runtime.block_on(self.inner.complete_pay())?;
        Ok(inner)
    }

    pub fn bolt11_invoice(&self) -> String {
        self.inner.bolt11_invoice.clone()
    }
}
