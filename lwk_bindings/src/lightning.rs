use std::{
    collections::HashMap,
    ops::ControlFlow,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    blockdata::address::BitcoinAddress, store::ForeignStoreLink, Address, Bolt11Invoice,
    ElectrumClient, EsploraClient, LightningPayment, LwkError, Mnemonic, Network,
};
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
    #[uniffi(default = false)]
    random_preimages: bool,
    /// Optional store for persisting swap data
    ///
    /// When set, swap data will be automatically persisted to the store after creation
    /// and on each state change. This enables automatic restoration of pending swaps.
    #[uniffi(default = None)]
    store: Option<Arc<ForeignStoreLink>>,
}

/// A session to pay and receive lightning payments.
///
/// Lightning payments are done via LBTC swaps using Boltz.
///
/// See `BoltzSessionBuilder` for various options to configure the session.
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
        write!(f, "{json}")
    }
}

#[derive(uniffi::Enum)]
pub enum PaymentState {
    Continue,
    Success,
    Failed,
}

/// Asset type for swap quotes
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapAsset {
    /// Lightning Bitcoin (for reverse/submarine swaps)
    Lightning,
    /// Onchain Bitcoin (for chain swaps)
    Onchain,
    /// Liquid Bitcoin (onchain)
    Liquid,
}

impl From<SwapAsset> for lwk_boltz::SwapAsset {
    fn from(asset: SwapAsset) -> Self {
        match asset {
            SwapAsset::Lightning => lwk_boltz::SwapAsset::Lightning,
            SwapAsset::Onchain => lwk_boltz::SwapAsset::Onchain,
            SwapAsset::Liquid => lwk_boltz::SwapAsset::Liquid,
        }
    }
}

/// Quote result containing fee breakdown for a swap
#[derive(uniffi::Record)]
pub struct Quote {
    /// Amount the user sends (before fees)
    pub send_amount: u64,
    /// Amount the user will receive after fees
    pub receive_amount: u64,
    /// Network/miner fee in satoshis
    pub network_fee: u64,
    /// Boltz service fee in satoshis
    pub boltz_fee: u64,
    /// Minimum amount for this swap pair
    pub min: u64,
    /// Maximum amount for this swap pair
    pub max: u64,
}

impl From<lwk_boltz::Quote> for Quote {
    fn from(quote: lwk_boltz::Quote) -> Self {
        Self {
            send_amount: quote.send_amount,
            receive_amount: quote.receive_amount,
            network_fee: quote.network_fee,
            boltz_fee: quote.boltz_fee,
            min: quote.min,
            max: quote.max,
        }
    }
}

/// Builder for creating swap quotes
#[derive(uniffi::Object)]
pub struct QuoteBuilder {
    inner: Mutex<Option<lwk_boltz::QuoteBuilder>>,
}

fn quote_builder_consumed() -> LwkError {
    "This quote builder has already been consumed".into()
}

#[uniffi::export]
impl QuoteBuilder {
    /// Set the source asset for the swap
    pub fn send(&self, asset: SwapAsset) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let builder = lock.take().ok_or_else(quote_builder_consumed)?;
        *lock = Some(builder.send(asset.into()));
        Ok(())
    }

    /// Set the destination asset for the swap
    pub fn receive(&self, asset: SwapAsset) -> Result<(), LwkError> {
        let mut lock = self.inner.lock()?;
        let builder = lock.take().ok_or_else(quote_builder_consumed)?;
        *lock = Some(builder.receive(asset.into()));
        Ok(())
    }

    /// Build the quote, calculating fees and receive amount
    pub fn build(&self) -> Result<Quote, LwkError> {
        let mut lock = self.inner.lock()?;
        let builder = lock.take().ok_or_else(quote_builder_consumed)?;
        let quote = builder.build()?;
        Ok(quote.into())
    }
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
            random_preimages: false,
            store: None,
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
        lwk_builder = lwk_builder.random_preimages(builder.random_preimages);

        if let Some(store) = builder.store.clone() {
            lwk_builder = lwk_builder.store(store);
        }

        let inner = lwk_builder
            .build_blocking()
            .map_err(|e| LwkError::Generic {
                msg: format!("Failed to create blocking lightning session: {e:?}"),
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
                status,
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
                msg: format!("Invoice failed: {e:?}"),
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
        refund_address: &BitcoinAddress,
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
        let response = self.inner.btc_to_lbtc(
            amount,
            refund_address.as_ref(),
            claim_address.as_ref(),
            webhook,
        )?;
        Ok(LockupResponse {
            inner: Mutex::new(Some(response)),
        })
    }

    /// Create an onchain swap to convert LBTC to BTC
    pub fn lbtc_to_btc(
        &self,
        amount: u64,
        refund_address: &Address,
        claim_address: &BitcoinAddress,
        webhook: Option<Arc<WebHook>>,
    ) -> Result<LockupResponse, LwkError> {
        let webhook = webhook
            .as_ref()
            .map(|w| lwk_boltz::Webhook::<ChainSwapStates> {
                url: w.url.to_string(),
                hash_swap_id: None,
                status: None,
            });

        let response = self.inner.lbtc_to_btc(
            amount,
            refund_address.as_ref(),
            claim_address.as_ref(),
            webhook,
        )?;
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

    /// Get the list of pending swap IDs from the store
    ///
    /// Returns an error if no store is configured.
    pub fn pending_swap_ids(&self) -> Result<Vec<String>, LwkError> {
        Ok(self.inner.pending_swap_ids()?)
    }

    /// Get the list of completed swap IDs from the store
    ///
    /// Returns an error if no store is configured.
    pub fn completed_swap_ids(&self) -> Result<Vec<String>, LwkError> {
        Ok(self.inner.completed_swap_ids()?)
    }

    /// Get the raw swap data (JSON) for a specific swap ID from the store
    ///
    /// Returns `None` if no store is configured or the swap doesn't exist.
    pub fn get_swap_data(&self, swap_id: String) -> Result<Option<String>, LwkError> {
        Ok(self.inner.get_swap_data(&swap_id)?)
    }

    /// Remove a swap from the store
    ///
    /// Returns `true` if the swap was removed, `false` if no store is configured.
    pub fn remove_swap(&self, swap_id: String) -> Result<bool, LwkError> {
        Ok(self.inner.remove_swap(&swap_id)?)
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

    /// Filter the swap list to only include restorable BTC to LBTC swaps
    pub fn restorable_btc_to_lbtc_swaps(
        &self,
        swap_list: &SwapList,
        claim_address: &Address,
        refund_address: &BitcoinAddress,
    ) -> Result<Vec<String>, LwkError> {
        let response = self.inner.restorable_btc_to_lbtc_swaps(
            &swap_list.inner,
            claim_address.as_ref(),
            refund_address.as_ref(),
        )?;
        let data = response
            .into_iter()
            .map(|e| self.inner.restore_lockup(e.into()))
            .map(|e| e.and_then(|e| e.serialize()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(data)
    }

    /// Filter the swap list to only include restorable LBTC to BTC swaps
    pub fn restorable_lbtc_to_btc_swaps(
        &self,
        swap_list: &SwapList,
        claim_address: &BitcoinAddress,
        refund_address: &Address,
    ) -> Result<Vec<String>, LwkError> {
        let response = self.inner.restorable_lbtc_to_btc_swaps(
            &swap_list.inner,
            claim_address.as_ref(),
            refund_address.as_ref(),
        )?;
        let data = response
            .into_iter()
            .map(|e| self.inner.restore_lockup(e.into()))
            .map(|e| e.and_then(|e| e.serialize()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(data)
    }

    /// Fetch informations, such as min and max amounts, about the reverse and submarine pairs from the boltz api.
    pub fn fetch_swaps_info(&self) -> Result<String, LwkError> {
        let (reverse, submarine, chain) = self.inner.fetch_swaps_info()?;
        let reverse_json = serde_json::to_value(&reverse)?;
        let submarine_json = serde_json::to_value(&submarine)?;
        let chain_json = serde_json::to_value(&chain)?;
        let mut result = HashMap::new();
        result.insert("reverse".to_string(), reverse_json);
        result.insert("submarine".to_string(), submarine_json);
        result.insert("chain".to_string(), chain_json);
        let result_json = serde_json::to_string(&result)?;
        Ok(result_json)
    }

    /// Refresh the cached pairs data from the Boltz API
    ///
    /// This updates the internal cache used by [`BoltzSession::quote()`].
    /// Call this if you need up-to-date fee information after the session was created.
    pub fn refresh_swap_info(&self) -> Result<(), LwkError> {
        self.inner.refresh_swap_info()?;
        Ok(())
    }

    /// Get the next index to use for deriving keypairs
    pub fn next_index_to_use(&self) -> u32 {
        self.inner.next_index_to_use()
    }

    /// Set the next index to use for deriving keypairs
    ///
    /// This may be necessary to handle multiple sessions with the same mnemonic.
    pub fn set_next_index_to_use(&self, next_index_to_use: u32) {
        self.inner.set_next_index_to_use(next_index_to_use);
    }

    /// Create a quote builder for calculating swap fees
    ///
    /// This uses the cached pairs data from session initialization.
    ///
    /// # Example
    /// ```ignore
    /// let builder = session.quote(25000);
    /// builder.send(SwapAsset::Lightning);
    /// builder.receive(SwapAsset::Liquid);
    /// let quote = builder.build()?;
    /// ```
    pub fn quote(&self, send_amount: u64) -> Arc<QuoteBuilder> {
        Arc::new(QuoteBuilder {
            inner: Mutex::new(Some(self.inner.quote(send_amount))),
        })
    }

    /// Create a quote builder for calculating send amount from desired receive amount
    ///
    /// This is the inverse of [`BoltzSession::quote()`] - given the amount you want
    /// to receive, it calculates how much you need to send.
    ///
    /// # Example
    /// ```ignore
    /// let builder = session.quote_receive(24887);
    /// builder.send(SwapAsset::Lightning);
    /// builder.receive(SwapAsset::Liquid);
    /// let quote = builder.build()?;
    /// // quote.send_amount will be 25000
    /// ```
    pub fn quote_receive(&self, receive_amount: u64) -> Arc<QuoteBuilder> {
        Arc::new(QuoteBuilder {
            inner: Mutex::new(Some(self.inner.quote_receive(receive_amount))),
        })
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
            .swap_id()
            .to_string())
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
            .uri_address()?;
        Ok(Arc::new(uri_address.into()))
    }
    pub fn uri_amount(&self) -> Result<u64, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .uri_amount())
    }

    /// The fee of the swap provider and the network fee
    ///
    /// It is equal to the amount requested onchain minus the amount of the bolt11 invoice
    pub fn fee(&self) -> Result<Option<u64>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .fee())
    }

    /// The fee of the swap provider
    ///
    /// It is equal to the invoice amount multiplied by the boltz fee rate.
    /// For example for paying an invoice of 1000 satoshi with a 0.1% rate would be 1 satoshi.
    pub fn boltz_fee(&self) -> Result<Option<u64>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .boltz_fee())
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
                *lock = Some(response);
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
            .swap_id()
            .to_string())
    }

    /// The fee of the swap provider and the network fee
    ///
    /// It is equal to the amount of the invoice minus the amount of the onchain transaction.
    pub fn fee(&self) -> Result<Option<u64>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .fee())
    }

    /// The fee of the swap provider
    ///
    /// It is equal to the invoice amount multiplied by the boltz fee rate.
    /// For example for receiving an invoice of 10000 satoshi with a 0.25% rate would be 25 satoshi.
    pub fn boltz_fee(&self) -> Result<Option<u64>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .boltz_fee())
    }

    /// The txid of the claim transaction of the swap
    pub fn claim_txid(&self) -> Result<Option<String>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .claim_txid()
            .map(|txid| txid.to_string()))
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
                *lock = Some(response);
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
            .swap_id()
            .to_string())
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

    /// The fee of the swap provider and the network fee
    ///
    /// It is equal to the amount requested minus the amount sent to the claim address.
    pub fn fee(&self) -> Result<Option<u64>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .fee())
    }

    /// The fee of the swap provider
    ///
    /// It is equal to the swap amount multiplied by the boltz fee rate.
    pub fn boltz_fee(&self) -> Result<Option<u64>, LwkError> {
        Ok(self
            .inner
            .lock()?
            .as_ref()
            .ok_or(LwkError::ObjectConsumed)?
            .boltz_fee())
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
