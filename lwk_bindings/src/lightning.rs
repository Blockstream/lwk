use std::{
    collections::HashMap,
    ops::ControlFlow,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    Address, Bolt11Invoice, ElectrumClient, EsploraClient, LightningPayment, LwkError, Mnemonic,
    Network,
};
use elements::bitcoin;
use log::{Level, Metadata, Record};
use lwk_boltz::{
    ChainSwapDataSerializable, ChainSwapStates, InvoiceDataSerializable,
    PreparePayDataSerializable, RevSwapStates, SubSwapStates,
};
use std::fmt;

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

/// A builder for the `BoltzSession`
#[derive(uniffi::Record)]
pub struct BoltzSessionBuilder {
    network: Arc<Network>,
    client: Arc<AnyClient>,
    #[uniffi(default = None)]
    timeout: Option<u64>,
    #[uniffi(default = None)]
    mnemonic: Option<Arc<Mnemonic>>,
    #[uniffi(default = None)]
    logging: Option<Arc<dyn Logging>>,
    #[uniffi(default = false)]
    polling: bool,
    #[uniffi(default = None)]
    timeout_advance: Option<u64>,
    #[uniffi(default = None)]
    next_index_to_use: Option<u32>,
    #[uniffi(default = None)]
    referral_id: Option<String>,
    #[uniffi(default = None)]
    bitcoin_electrum_client_url: Option<String>,
}

/// A session to pay and receive lightning payments.
///
/// Lightning payments are done via LBTC swaps using Boltz.
#[derive(uniffi::Object)]
pub struct BoltzSession {
    inner: lwk_boltz::blocking::BoltzSession,
    #[allow(dead_code)]
    logging: Option<Arc<dyn Logging>>,
}

#[derive(uniffi::Object)]
pub struct PreparePayResponse {
    /// Using Option to allow consuming the inner value when complete_pay is called
    inner: Mutex<Option<lwk_boltz::blocking::PreparePayResponse>>,
}

#[derive(uniffi::Object)]
pub struct WebHook {
    url: String,
    status: Vec<String>,
}

#[derive(uniffi::Object)]
pub struct InvoiceResponse {
    /// Using Option to allow consuming the inner value when complete_pay is called
    inner: Mutex<Option<lwk_boltz::blocking::InvoiceResponse>>,
}

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct SwapList {
    inner: Vec<lwk_boltz::SwapRestoreResponse>,
}

#[derive(uniffi::Object)]
pub struct LockupResponse {
    inner: Mutex<Option<lwk_boltz::blocking::LockupResponse>>,
}

impl fmt::Display for SwapList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = serde_json::to_string(&self.inner).map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

#[derive(uniffi::Enum)]
pub enum PaymentState {
    Continue,
    Success,
    Failed,
}

#[derive(uniffi::Object)]
pub enum AnyClient {
    Electrum(Arc<ElectrumClient>),
    Esplora(Arc<EsploraClient>),
}

#[uniffi::export]
impl AnyClient {
    #[uniffi::constructor]
    pub fn from_electrum(client: Arc<ElectrumClient>) -> Self {
        AnyClient::Electrum(client)
    }

    #[uniffi::constructor]
    pub fn from_esplora(client: Arc<EsploraClient>) -> Self {
        AnyClient::Esplora(client)
    }
}

#[uniffi::export]
impl BoltzSession {
    /// Create the lightning session with default settings
    ///
    /// This uses default timeout and generates a random mnemonic.
    /// For custom configuration, use [`BoltzSession::from_builder()`] instead.
    #[uniffi::constructor]
    pub fn new(network: &Network, client: &AnyClient) -> Result<Self, LwkError> {
        let client_arc = match client {
            AnyClient::Electrum(c) => Arc::new(AnyClient::Electrum(c.clone())),
            AnyClient::Esplora(c) => Arc::new(AnyClient::Esplora(c.clone())),
        };
        let builder = BoltzSessionBuilder {
            network: Arc::new(*network),
            client: client_arc,
            timeout: None,
            mnemonic: None,
            logging: None,
            polling: false,
            timeout_advance: None,
            next_index_to_use: None,
            referral_id: None,
            bitcoin_electrum_client_url: None,
        };
        Self::from_builder(builder)
    }

    /// Create the lightning session from a builder
    #[uniffi::constructor]
    pub fn from_builder(builder: BoltzSessionBuilder) -> Result<Self, LwkError> {
        // Validate the logger by attempting a test call
        if let Some(ref logger_impl) = builder.logging {
            // Test the logger with a dummy message to catch issues early
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                logger_impl.log(LogLevel::Debug, "Logger validation test".to_string());
            })).map_err(|_| LwkError::Generic {
                msg: "Logger validation failed. Please ensure you pass an instance of a class that implements the Logging trait, not the class itself.".to_string(),
            })?;
        }

        // Set up the custom logger if provided
        if let Some(ref logger_impl) = builder.logging {
            let bridge = LoggingBridge {
                inner: logger_impl.clone(),
            };
            // Try to set the logger. This can only be done once globally.
            // If it fails (logger already set), we silently continue.
            let _ = log::set_boxed_logger(Box::new(bridge))
                .map(|()| log::set_max_level(log::LevelFilter::Trace));
        }
        log::info!("Creating lightning session from builder");

        let network_value = builder.network.as_ref().into();

        let client = match builder.client.as_ref() {
            AnyClient::Electrum(client) => {
                let boltz_client = lwk_boltz::clients::ElectrumClient::from_client(
                    client.clone_client().expect("TODO"),
                    network_value,
                );
                lwk_boltz::clients::AnyClient::Electrum(Arc::new(boltz_client))
            }
            AnyClient::Esplora(client) => {
                let boltz_client = lwk_boltz::clients::EsploraClient::from_client(
                    Arc::new(client.clone_async_client().expect("TODO")),
                    network_value,
                );
                lwk_boltz::clients::AnyClient::Esplora(Arc::new(boltz_client))
            }
        };

        let mut lwk_builder = lwk_boltz::BoltzSession::builder(network_value, client);
        if let Some(timeout_secs) = builder.timeout {
            lwk_builder = lwk_builder.create_swap_timeout(Duration::from_secs(timeout_secs));
        }
        if let Some(mnemonic) = builder.mnemonic {
            lwk_builder = lwk_builder.mnemonic(mnemonic.inner());
        }
        lwk_builder = lwk_builder.polling(builder.polling);
        if let Some(timeout_advance_secs) = builder.timeout_advance {
            lwk_builder = lwk_builder.timeout_advance(Duration::from_secs(timeout_advance_secs));
        }
        if let Some(next_index_to_use) = builder.next_index_to_use {
            lwk_builder = lwk_builder.next_index_to_use(next_index_to_use);
        }
        if let Some(referral_id) = builder.referral_id {
            lwk_builder = lwk_builder.referral_id(referral_id);
        }

        let inner = lwk_builder
            .build_blocking()
            .map_err(|e| LwkError::Generic {
                msg: format!("Failed to create blocking lightning session: {:?}", e),
            })?;
        Ok(Self {
            inner,
            logging: builder.logging,
        })
    }

    /// Prepare to pay a bolt11 invoice
    pub fn prepare_pay(
        &self,
        lightning_payment: &LightningPayment,
        refund_address: &Address,
        webhook: Option<Arc<WebHook>>,
    ) -> Result<PreparePayResponse, LwkError> {
        let status = webhook
            .as_ref()
            .filter(|w| !w.status.is_empty())
            .map(|w| {
                w.status
                    .iter()
                    .map(|s| s.parse::<SubSwapStates>())
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|_| LwkError::Generic {
                msg: "Invalid status".to_string(),
            })?;
        let webhook = webhook
            .as_ref()
            .map(|w| lwk_boltz::Webhook::<SubSwapStates> {
                url: w.url.to_string(),
                hash_swap_id: None,
                status: status,
            });
        let response =
            self.inner
                .prepare_pay(lightning_payment.as_ref(), refund_address.as_ref(), webhook)?;

        Ok(PreparePayResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Restore a payment from its serialized data see `PreparePayResponse::serialize`
    pub fn restore_prepare_pay(&self, data: &str) -> Result<PreparePayResponse, LwkError> {
        let data = PreparePayDataSerializable::deserialize(data)?;
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
        webhook: Option<Arc<WebHook>>,
    ) -> Result<InvoiceResponse, LwkError> {
        let status = webhook
            .as_ref()
            .filter(|w| !w.status.is_empty())
            .map(|w| {
                w.status
                    .iter()
                    .map(|s| s.parse::<RevSwapStates>())
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|_| LwkError::Generic {
                msg: "Invalid status".to_string(),
            })?;
        let webhook = webhook
            .as_ref()
            .map(|w| lwk_boltz::Webhook::<RevSwapStates> {
                url: w.url.to_string(),
                hash_swap_id: None,
                status,
            });
        let response = self
            .inner
            .invoice(amount, description, claim_address.as_ref(), webhook)
            .map_err(|e| LwkError::Generic {
                msg: format!("Invoice failed: {:?}", e),
            })?;

        Ok(InvoiceResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Restore an invoice flow from its serialized data see `InvoiceResponse::serialize`
    pub fn restore_invoice(&self, data: &str) -> Result<InvoiceResponse, LwkError> {
        let data: InvoiceDataSerializable = serde_json::from_str(data)?;
        let response = self.inner.restore_invoice(data)?;
        Ok(InvoiceResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Create an onchain swap to convert BTC to LBTC
    pub fn btc_to_lbtc(
        &self,
        amount: u64,
        refund_address: &str, // TODO: convert to BitcoinAddress?
        claim_address: &Address,
        webhook: Option<Arc<WebHook>>,
    ) -> Result<LockupResponse, LwkError> {
        let webhook = webhook
            .as_ref()
            .map(|w| lwk_boltz::Webhook::<ChainSwapStates> {
                url: w.url.to_string(),
                hash_swap_id: None,
                status: None,
            });
        let refund_address = bitcoin::Address::from_str(refund_address)
            .expect("TODO")
            .assume_checked();
        let response =
            self.inner
                .btc_to_lbtc(amount, &refund_address, claim_address.as_ref(), webhook)?;
        Ok(LockupResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Create an onchain swap to convert LBTC to BTC
    pub fn lbtc_to_btc(
        &self,
        amount: u64,
        refund_address: &Address,
        claim_address: &str,
        webhook: Option<Arc<WebHook>>,
    ) -> Result<LockupResponse, LwkError> {
        let webhook = webhook
            .as_ref()
            .map(|w| lwk_boltz::Webhook::<ChainSwapStates> {
                url: w.url.to_string(),
                hash_swap_id: None,
                status: None,
            });

        let claim_address = bitcoin::Address::from_str(claim_address)
            .expect("TODO")
            .assume_checked();
        let response =
            self.inner
                .lbtc_to_btc(amount, refund_address.as_ref(), &claim_address, webhook)?;
        Ok(LockupResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Restore an onchain swap from its serialized data see `LockupResponse::serialize`
    pub fn restore_lockup(&self, data: &str) -> Result<LockupResponse, LwkError> {
        let data = ChainSwapDataSerializable::deserialize(data)?;
        let response = self.inner.restore_lockup(data)?;
        Ok(LockupResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Generate a rescue file with lightning session mnemonic.
    ///
    /// The rescue file is a JSON file that contains the swaps mnemonic.
    /// It can be used on the Boltz web app to bring non terminated swaps to completition.
    pub fn rescue_file(&self) -> Result<String, LwkError> {
        let rescue_file = self.inner.rescue_file();
        let rescue_file_json = serde_json::to_string(&rescue_file)?;
        Ok(rescue_file_json)
    }

    /// Returns a the list of all the swaps ever done with the session mnemonic.
    ///
    /// The object returned can be converted to a json String with toString()
    pub fn swap_restore(&self) -> Result<SwapList, LwkError> {
        let response = self.inner.swap_restore()?;
        Ok(SwapList { inner: response })
    }

    /// Filter the swap list to only include restorable reverse swaps
    pub fn restorable_reverse_swaps(
        &self,
        swap_list: &SwapList,
        claim_address: &Address,
    ) -> Result<Vec<String>, LwkError> {
        let response = self
            .inner
            .restorable_reverse_swaps(&swap_list.inner, claim_address.as_ref())?;
        let data = response
            .into_iter()
            .map(|e| self.inner.restore_invoice(e.into()))
            .map(|e| e.and_then(|e| e.serialize()))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(data)
    }

    /// Filter the swap list to only include restorable submarine swaps
    pub fn restorable_submarine_swaps(
        &self,
        swap_list: &SwapList,
        refund_address: &Address,
    ) -> Result<Vec<String>, LwkError> {
        let response = self
            .inner
            .restorable_submarine_swaps(&swap_list.inner, refund_address.as_ref())?;
        let data = response
            .into_iter()
            .map(|e| self.inner.restore_prepare_pay(e.into()))
            .map(|e| e.and_then(|e| e.serialize()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(data)
    }

    /// Fetch informations, such as min and max amounts, about the reverse and submarine pairs from the boltz api.
    pub fn fetch_swaps_info(&self) -> Result<String, LwkError> {
        let (reverse, submarine) = self.inner.fetch_swaps_info()?;
        let reverse_json = serde_json::to_value(&reverse)?;
        let submarine_json = serde_json::to_value(&submarine)?;
        let mut result = HashMap::new();
        result.insert("reverse".to_string(), reverse_json);
        result.insert("submarine".to_string(), submarine_json);
        let result_json = serde_json::to_string(&result)?;
        Ok(result_json)
    }

    /// Get the next index to use for deriving keypairs
    ///
    /// Users should call this after each created swap and store in persisting memory, so that is passed
    /// when creating a new session with the same mnemonic.
    pub fn next_index_to_use(&self) -> u32 {
        self.inner.next_index_to_use()
    }
}

#[uniffi::export]
impl PreparePayResponse {
    pub fn complete_pay(&self) -> Result<bool, LwkError> {
        let mut lock = self.inner.lock()?;
        let response = lock.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(response.complete_pay()?)
    }

    pub fn swap_id(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
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
            .ok_or(LwkError::ObjectConsumed)?
            .serialize()?)
    }

    pub fn uri(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .uri())
    }

    pub fn uri_address(&self) -> Result<Arc<Address>, LwkError> {
        let uri_address = self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .uri_address();
        Address::new(&uri_address)
    }
    pub fn uri_amount(&self) -> Result<u64, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .uri_amount())
    }

    pub fn advance(&self) -> Result<PaymentState, LwkError> {
        let mut lock = self.inner.lock()?;
        let mut response = lock.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(match response.advance() {
            Ok(ControlFlow::Continue(_update)) => {
                *lock = Some(response);
                PaymentState::Continue
            }
            Ok(ControlFlow::Break(update)) => {
                if update {
                    PaymentState::Success
                } else {
                    PaymentState::Failed
                }
            }
            Err(lwk_boltz::Error::NoBoltzUpdate) => {
                *lock = Some(response);
                return Err(LwkError::NoBoltzUpdate);
            }
            Err(e) => return Err(e.into()),
        })
    }
}

#[uniffi::export]
impl InvoiceResponse {
    pub fn bolt11_invoice(&self) -> Result<Bolt11Invoice, LwkError> {
        let bolt11_invoice = self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .bolt11_invoice();
        Ok(Bolt11Invoice::from(bolt11_invoice))
    }

    pub fn swap_id(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
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
            .ok_or(LwkError::ObjectConsumed)?
            .serialize()?)
    }

    pub fn complete_pay(&self) -> Result<bool, LwkError> {
        let mut lock = self.inner.lock()?;
        let response = lock.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(response.complete_pay()?)
    }

    pub fn advance(&self) -> Result<PaymentState, LwkError> {
        let mut lock = self.inner.lock()?;
        let mut response = lock.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(match response.advance() {
            Ok(ControlFlow::Continue(_update)) => {
                *lock = Some(response);
                PaymentState::Continue
            }
            Ok(ControlFlow::Break(update)) => {
                if update {
                    PaymentState::Success
                } else {
                    PaymentState::Failed
                }
            }
            Err(lwk_boltz::Error::NoBoltzUpdate) => {
                *lock = Some(response);
                return Err(LwkError::NoBoltzUpdate);
            }
            Err(e) => return Err(e.into()),
        })
    }
}

#[uniffi::export]
impl LockupResponse {
    pub fn swap_id(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .swap_id())
    }

    pub fn lockup_address(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .lockup_address()
            .to_string())
    }

    pub fn expected_amount(&self) -> Result<u64, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .expected_amount())
    }

    pub fn chain_from(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .chain_from()
            .to_string())
    }

    pub fn chain_to(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .chain_to()
            .to_string())
    }

    pub fn advance(&self) -> Result<PaymentState, LwkError> {
        let mut lock = self.inner.lock()?;
        let mut response = lock.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(match response.advance() {
            Ok(ControlFlow::Continue(_update)) => {
                *lock = Some(response);
                PaymentState::Continue
            }
            Ok(ControlFlow::Break(update)) => {
                if update {
                    PaymentState::Success
                } else {
                    PaymentState::Failed
                }
            }
            Err(lwk_boltz::Error::NoBoltzUpdate) => {
                *lock = Some(response);
                return Err(LwkError::NoBoltzUpdate);
            }
            Err(e) => return Err(e.into()),
        })
    }

    pub fn complete(&self) -> Result<bool, LwkError> {
        let mut lock = self.inner.lock()?;
        let response = lock.take().ok_or(LwkError::ObjectConsumed)?;
        Ok(response.complete()?)
    }

    pub fn serialize(&self) -> Result<String, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .serialize()?)
    }
}

#[uniffi::export]
impl WebHook {
    #[uniffi::constructor]
    pub fn new(url: String, status: Vec<String>) -> Arc<Self> {
        Arc::new(Self { url, status })
    }
}
