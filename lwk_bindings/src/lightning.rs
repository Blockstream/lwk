use std::sync::{Arc, Mutex};

use crate::{ElectrumClient, LwkError, Network};

/// A session to pay and receive lightning payments.
///
/// Lightning payments are done via LBTC swaps using Boltz.
#[derive(uniffi::Object)]
pub struct LighthingSession {
    inner: lwk_boltz::LighthingSession,
}

#[derive(uniffi::Object)]
pub struct PreparePayResponse {
    /// Using Option to allow consuming the inner value when complete_pay is called
    inner: Mutex<Option<lwk_boltz::PreparePayResponse>>,
}

#[derive(uniffi::Object)]
pub struct InvoiceResponse {
    /// Using Option to allow consuming the inner value when complete_pay is called
    inner: Mutex<Option<lwk_boltz::InvoiceResponse>>,
}

impl From<lwk_boltz::PreparePayResponse> for PreparePayResponse {
    fn from(inner: lwk_boltz::PreparePayResponse) -> Self {
        Self {
            inner: Mutex::new(Some(inner)),
        }
    }
}

impl From<lwk_boltz::InvoiceResponse> for InvoiceResponse {
    fn from(inner: lwk_boltz::InvoiceResponse) -> Self {
        Self {
            inner: Mutex::new(Some(inner)),
        }
    }
}

#[uniffi::export]
impl LighthingSession {
    /// Create the lightning session
    ///
    /// Note the passed `ElectrumClient` should not be referenced elsewhere and it will be consumed
    /// by this method (not available after this call).
    #[uniffi::constructor]
    pub fn new(network: &Network, client: Arc<ElectrumClient>) -> Result<Self, LwkError> {
        // Try to unwrap the Arc to get owned ElectrumClient
        let inner_client = Arc::try_unwrap(client)
            .map_err(|_| LwkError::Generic {
                msg: "ElectrumClient is still referenced elsewhere".to_string(),
            })?
            .into_inner()
            .map_err(|_| LwkError::Generic {
                msg: "ElectrumClient mutex is poisoned".to_string(),
            })?;

        let client = lwk_boltz::clients::ElectrumClient::from_client(inner_client, network.into());
        let inner = lwk_boltz::LighthingSession::new(network.into(), client);
        Ok(Self { inner })
    }

    /// Prepare to pay a bolt11 invoice
    pub async fn prepare_pay(
        &self,
        invoice: &str,
        // _refund_address: &str, // TODO
    ) -> Result<PreparePayResponse, LwkError> {
        self.inner
            .prepare_pay(invoice)
            .await
            .map(Into::into)
            .map_err(|e| LwkError::Generic {
                msg: format!("Prepare pay failed: {:?}", e),
            })
    }

    /// Create a new invoice for a given amount and a claim address to receive the payment
    pub async fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: &str,
    ) -> Result<InvoiceResponse, LwkError> {
        self.inner
            .invoice(amount, description, claim_address.to_string())
            .await
            .map(Into::into)
            .map_err(|e| LwkError::Generic {
                msg: format!("Invoice failed: {:?}", e),
            })
    }
}

#[uniffi::export]
impl PreparePayResponse {
    pub async fn complete_pay(&self) -> Result<bool, LwkError> {
        // Extract the inner value and drop the lock before awaiting
        let inner = {
            let mut lock = self.inner.lock()?;
            lock.take().ok_or_else(|| LwkError::Generic {
                msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
            })?
        };
        // Now we can await without holding the lock
        inner.complete_pay().await.map_err(|e| LwkError::Generic {
            msg: format!("Complete pay failed: {:?}", e),
        })
    }
}

#[uniffi::export]
impl InvoiceResponse {
    pub async fn complete_pay(&self) -> Result<bool, LwkError> {
        // Extract the inner value and drop the lock before awaiting
        let inner = {
            let mut lock = self.inner.lock()?;
            lock.take().ok_or_else(|| LwkError::Generic {
                msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
            })?
        };
        // Now we can await without holding the lock
        inner.complete_pay().await.map_err(|e| LwkError::Generic {
            msg: format!("Complete pay failed: {:?}", e),
        })
    }
}
