use std::sync::Mutex;

use crate::{LwkError, Network};
use tokio::runtime::Runtime;

/// A session to pay and receive lightning payments.
///
/// Lightning payments are done via LBTC swaps using Boltz.
#[derive(uniffi::Object)]
pub struct LightningSession {
    inner: lwk_boltz::LightningSession,
    runtime: Runtime,
}

#[derive(uniffi::Object)]
pub struct PreparePayResponse {
    /// Using Option to allow consuming the inner value when complete_pay is called
    inner: Mutex<Option<lwk_boltz::PreparePayResponse>>,
    runtime: Runtime,
}

#[derive(uniffi::Object)]
pub struct InvoiceResponse {
    /// Using Option to allow consuming the inner value when complete_pay is called
    inner: Mutex<Option<lwk_boltz::InvoiceResponse>>,
    runtime: Runtime,
}

#[uniffi::export]
impl LightningSession {
    /// Create the lightning session
    ///
    /// TODO: is there a way to pass the electrum client directly? cannot use Arc::try_unwrap because uniffi keeps references around
    #[uniffi::constructor]
    pub fn new(
        network: &Network,
        electrum_url: &str,
        tls: bool,
        validate_domain: bool,
    ) -> Result<Self, LwkError> {
        let url = lwk_wollet::ElectrumUrl::new(electrum_url, tls, validate_domain)
            .map_err(lwk_wollet::Error::Url)?;
        let client = lwk_wollet::ElectrumClient::new(&url)?;
        let client = lwk_boltz::clients::ElectrumClient::from_client(client, network.into());

        let runtime = Runtime::new().map_err(|e| LwkError::Generic {
            msg: format!("Failed to create tokio runtime: {}", e),
        })?;

        // Enter the runtime context before creating the inner session
        // because lwk_boltz::LightningSession::new calls tokio::spawn
        let inner = {
            let _guard = runtime.enter();
            lwk_boltz::LightningSession::new(network.into(), client)
        };

        Ok(Self { inner, runtime })
    }

    /// Prepare to pay a bolt11 invoice
    pub fn prepare_pay(
        &self,
        invoice: &str,
        // _refund_address: &str, // TODO
    ) -> Result<PreparePayResponse, LwkError> {
        let response = self
            .runtime
            .block_on(self.inner.prepare_pay(invoice))
            .map_err(|e| LwkError::Generic {
                msg: format!("Prepare pay failed: {:?}", e),
            })?;

        let runtime = Runtime::new().map_err(|e| LwkError::Generic {
            msg: format!("Failed to create tokio runtime: {}", e),
        })?;

        Ok(PreparePayResponse {
            inner: Mutex::new(Some(response)),
            runtime,
        })
    }

    /// Create a new invoice for a given amount and a claim address to receive the payment
    pub fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: &str,
    ) -> Result<InvoiceResponse, LwkError> {
        let response = self
            .runtime
            .block_on(
                self.inner
                    .invoice(amount, description, claim_address.to_string()),
            )
            .map_err(|e| LwkError::Generic {
                msg: format!("Invoice failed: {:?}", e),
            })?;

        let runtime = Runtime::new().map_err(|e| LwkError::Generic {
            msg: format!("Failed to create tokio runtime: {}", e),
        })?;

        Ok(InvoiceResponse {
            inner: Mutex::new(Some(response)),
            runtime,
        })
    }
}

#[uniffi::export]
impl PreparePayResponse {
    pub fn complete_pay(&self) -> Result<bool, LwkError> {
        // Extract the inner value and drop the lock before awaiting
        let inner = {
            let mut lock = self.inner.lock()?;
            lock.take().ok_or_else(|| LwkError::Generic {
                msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
            })?
        };
        // Now we can await without holding the lock
        self.runtime
            .block_on(inner.complete_pay())
            .map_err(|e| LwkError::Generic {
                msg: format!("Complete pay failed: {:?}", e),
            })
    }
}

#[uniffi::export]
impl InvoiceResponse {
    pub fn bolt11_invoice(&self) -> Result<String, LwkError> {
        Ok({
            let lock = self.inner.lock()?;
            lock.as_ref()
                .ok_or_else(|| LwkError::Generic {
                    msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
                })?
                .bolt11_invoice
                .clone()
        })
    }

    pub fn complete_pay(&self) -> Result<bool, LwkError> {
        // Extract the inner value and drop the lock before awaiting
        let inner = {
            let mut lock = self.inner.lock()?;
            lock.take().ok_or_else(|| LwkError::Generic {
                msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
            })?
        };
        // Now we can await without holding the lock
        self.runtime
            .block_on(inner.complete_pay())
            .map_err(|e| LwkError::Generic {
                msg: format!("Complete pay failed: {:?}", e),
            })
    }
}
