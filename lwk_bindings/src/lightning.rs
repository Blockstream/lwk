use std::{
    ops::ControlFlow,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{Address, Bolt11Invoice, ElectrumClient, LwkError, Network};
use log::{Level, Metadata, Record};

/// Log level for logging messages
#[derive(uniffi::Enum)]
pub enum LogLevel {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warn,
    /// Error level
    Error,
}

/// An exported trait for handling logging messages.
///
/// Implement this trait to receive and handle logging messages from the lightning session.
#[uniffi::export(with_foreign)]
pub trait Logging: Send + Sync {
    /// Log a message with the given level
    fn log(&self, level: LogLevel, message: String);
}

/// An object to define logging at the caller level
#[derive(uniffi::Object)]
pub struct LoggingLink {
    #[allow(dead_code)]
    pub(crate) inner: Arc<dyn Logging>,
}

#[uniffi::export]
impl LoggingLink {
    /// Create a new `LoggingLink`
    #[uniffi::constructor]
    pub fn new(logging: Arc<dyn Logging>) -> Self {
        Self { inner: logging }
    }
}

/// Bridge logger that forwards log messages to our custom Logging trait
struct LoggingBridge {
    inner: Arc<dyn Logging>,
}

impl log::Log for LoggingBridge {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let level = match record.level() {
            Level::Error => LogLevel::Error,
            Level::Warn => LogLevel::Warn,
            Level::Info => LogLevel::Info,
            Level::Debug => LogLevel::Debug,
            Level::Trace => LogLevel::Debug, // Map Trace to Debug
        };

        let message = format!("{}", record.args());
        self.inner.log(level, message);
    }

    fn flush(&self) {}
}

/// A session to pay and receive lightning payments.
///
/// Lightning payments are done via LBTC swaps using Boltz.
#[derive(uniffi::Object)]
pub struct LightningSession {
    inner: lwk_boltz::blocking::LightningSession,
    #[allow(dead_code)]
    logging: Option<Arc<dyn Logging>>,
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

#[derive(uniffi::Enum)]
pub enum PaymentState {
    Continue,
    Success,
    Failed,
}

#[uniffi::export]
impl LightningSession {
    /// Create the lightning session
    ///
    /// If a `logging` implementation is provided, it will be set as the global logger
    /// to receive log messages from the lightning operations. Note that the global
    /// logger can only be set once - if a logger is already set, the new one will be ignored.
    ///
    /// TODO: is there a way to pass the electrum client directly? cannot use Arc::try_unwrap because uniffi keeps references around
    #[uniffi::constructor]
    pub fn new(
        network: &Network,
        client: &ElectrumClient,
        timeout: Option<u64>,
        logging: Option<Arc<dyn Logging>>,
    ) -> Result<Self, LwkError> {
        // Set up the custom logger if provided
        if let Some(ref logger_impl) = logging {
            let bridge = LoggingBridge {
                inner: logger_impl.clone(),
            };
            // Try to set the logger. This can only be done once globally.
            // If it fails (logger already set), we silently continue.
            let _ = log::set_boxed_logger(Box::new(bridge))
                .and_then(|()| Ok(log::set_max_level(log::LevelFilter::Trace)));
        }
        log::info!("Creating lightning session");

        let network_value = network.into();
        // Transform lwk_bindings::ElectrumClient into lwk_boltz::clients::ElectrumClient
        let inner_client = client.clone_client()?;
        let boltz_client =
            lwk_boltz::clients::ElectrumClient::from_client(inner_client, network_value);
        let inner = lwk_boltz::blocking::LightningSession::new(
            network_value,
            Arc::new(boltz_client),
            timeout.map(Duration::from_secs),
        )
        .map_err(|e| LwkError::Generic {
            msg: format!("Failed to create blocking lightning session: {:?}", e),
        })?;
        Ok(Self { inner, logging })
    }

    /// Prepare to pay a bolt11 invoice
    pub fn prepare_pay(
        &self,
        invoice: &Bolt11Invoice,
        refund_address: &Address,
    ) -> Result<PreparePayResponse, LwkError> {
        let response = self
            .inner
            .prepare_pay(invoice.as_ref(), refund_address.as_ref())?;

        Ok(PreparePayResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Restore a payment from its serialized data see `PreparePayResponse::serialize`
    pub fn restore_prepare_pay(&self, data: &str) -> Result<PreparePayResponse, LwkError> {
        let response = self.inner.restore_prepare_pay(data)?;
        Ok(PreparePayResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Create a new invoice for a given amount and a claim address to receive the payment
    pub fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: &Address,
    ) -> Result<InvoiceResponse, LwkError> {
        let response = self
            .inner
            .invoice(amount, description, claim_address.as_ref())
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
        let response = lock.take().ok_or_else(|| LwkError::Generic {
            msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
        })?;
        Ok(response.complete_pay()?)
    }

    pub fn swap_id(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
            })?
            .swap_id())
    }

    /// Serialize the prepare pay response data to a json string
    ///
    /// This can be used to restore the prepare pay response after a crash
    pub fn serialize(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
            })?
            .serialize()?)
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

    pub fn uri_address(&self) -> Result<Arc<Address>, LwkError> {
        let uri_address = self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
            })?
            .uri_address();
        Ok(Address::new(&uri_address)?)
    }
    pub fn uri_amount(&self) -> Result<u64, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
            })?
            .uri_amount())
    }

    pub fn advance(&self) -> Result<PaymentState, LwkError> {
        let mut lock = self.inner.lock()?;
        let mut response = lock.take().ok_or_else(|| LwkError::Generic {
            msg: "This PreparePayResponse already called complete_pay or errored".to_string(),
        })?;
        let control_flow = response.advance()?;
        let result = match control_flow {
            ControlFlow::Continue(_update) => PaymentState::Continue,
            ControlFlow::Break(update) => {
                if update {
                    PaymentState::Success
                } else {
                    PaymentState::Failed
                }
            }
        };
        *lock = Some(response);
        Ok(result)
    }
}

#[uniffi::export]
impl InvoiceResponse {
    pub fn bolt11_invoice(&self) -> Result<Bolt11Invoice, LwkError> {
        let bolt11_invoice = self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
            })?
            .bolt11_invoice();
        Ok(Bolt11Invoice::from(bolt11_invoice))
    }

    pub fn swap_id(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
            })?
            .swap_id())
    }

    /// Serialize the prepare pay response data to a json string
    ///
    /// This can be used to restore the prepare pay response after a crash
    pub fn serialize(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or_else(|| LwkError::Generic {
                msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
            })?
            .serialize()?)
    }

    pub fn complete_pay(&self) -> Result<bool, LwkError> {
        let mut lock = self.inner.lock()?;
        let response = lock.take().ok_or_else(|| LwkError::Generic {
            msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
        })?;
        Ok(response.complete_pay()?)
    }

    pub fn advance(&self) -> Result<PaymentState, LwkError> {
        let mut lock = self.inner.lock()?;
        let mut response = lock.take().ok_or_else(|| LwkError::Generic {
            msg: "This InvoiceResponse already called complete_pay or errored".to_string(),
        })?;
        let control_flow = response.advance()?;
        let result = match control_flow {
            ControlFlow::Continue(_update) => PaymentState::Continue,
            ControlFlow::Break(update) => {
                if update {
                    PaymentState::Success
                } else {
                    PaymentState::Failed
                }
            }
        };
        *lock = Some(response);
        Ok(result)
    }
}
