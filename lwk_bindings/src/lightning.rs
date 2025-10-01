use std::sync::Mutex;

use crate::{LwkError, Network};

/// A session to pay and receive lightning payments.
///
/// Lightning payments are done via LBTC swaps using Boltz.
#[derive(uniffi::Object)]
pub struct LightningSession {
    inner: lwk_boltz::blocking::LightningSession,
}

#[derive(uniffi::Object)]
pub struct PreparePayResponse {
    /// Using Option to allow consuming the inner value when complete_pay is called
    inner: Mutex<Option<lwk_boltz::blocking::PreparePayResponse>>,
}

#[derive(uniffi::Object)]
pub struct InvoiceResponse {
    /// Using Option to allow consuming the inner value when complete_pay is called
    inner: Mutex<Option<lwk_boltz::blocking::InvoiceResponse>>,
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
        let async_session = lwk_boltz::LightningSession::new(network.into(), client);
        let inner = lwk_boltz::blocking::LightningSession::new(async_session).map_err(|e| {
            LwkError::Generic {
                msg: format!("Failed to create blocking lightning session: {:?}", e),
            }
        })?;
        Ok(Self { inner })
    }

    /// Prepare to pay a bolt11 invoice
    pub fn prepare_pay(
        &self,
        invoice: &str,
        // _refund_address: &str, // TODO
    ) -> Result<PreparePayResponse, LwkError> {
        let response = self
            .inner
            .prepare_pay(invoice)
            .map_err(|e| LwkError::Generic {
                msg: format!("Prepare pay failed: {:?}", e),
            })?;

        Ok(PreparePayResponse {
            inner: Mutex::new(Some(response)),
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
            .inner
            .invoice(amount, description, claim_address.to_string())
            .map_err(|e| LwkError::Generic {
                msg: format!("Invoice failed: {:?}", e),
            })?;

        Ok(InvoiceResponse {
            inner: Mutex::new(Some(response)),
        })
    }
}

#[uniffi::export]
impl PreparePayResponse {
    pub fn complete_pay(&self) -> Result<bool, LwkError> {
        let mut lock = self.inner.lock()?;
        lock.take()
            .ok_or_else(|| LwkError::Generic {
                msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
            })?
            .complete_pay()
            .map_err(|e| LwkError::Generic {
                msg: format!("Complete pay failed: {:?}", e),
            })
    }

    pub fn uri(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
            })?
            .uri())
    }
}

#[uniffi::export]
impl InvoiceResponse {
    pub fn bolt11_invoice(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
            })?
            .bolt11_invoice())
    }

    pub fn complete_pay(&self) -> Result<bool, LwkError> {
        let mut lock = self.inner.lock()?;
        lock.take()
            .ok_or_else(|| LwkError::Generic {
                msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
            })?
            .complete_pay()
            .map_err(|e| LwkError::Generic {
                msg: format!("Complete pay failed: {:?}", e),
            })
    }
}
