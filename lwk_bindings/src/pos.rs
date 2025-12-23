use std::sync::Arc;

use crate::{CurrencyCode, LwkError, WolletDescriptor};

/// POS (Point of Sale) configuration for encoding/decoding
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone)]
#[uniffi::export(Display, Eq)]
pub struct POSConfig {
    pub(crate) inner: lwk_wollet::POSConfig,
}

impl std::fmt::Display for POSConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "POSConfig(descriptor: {}, currency: {})",
            self.descriptor(),
            self.currency()
        )
    }
}

impl From<lwk_wollet::POSConfig> for POSConfig {
    fn from(inner: lwk_wollet::POSConfig) -> Self {
        Self { inner }
    }
}

impl From<POSConfig> for lwk_wollet::POSConfig {
    fn from(value: POSConfig) -> Self {
        value.inner
    }
}

impl From<&POSConfig> for lwk_wollet::POSConfig {
    fn from(value: &POSConfig) -> Self {
        value.inner.clone()
    }
}

#[uniffi::export]
impl POSConfig {
    /// Create a new POS configuration
    #[uniffi::constructor]
    pub fn new(descriptor: &WolletDescriptor, currency: &CurrencyCode) -> Arc<POSConfig> {
        let inner =
            lwk_wollet::POSConfig::new(descriptor.as_ref().clone(), currency.as_ref().clone());
        Arc::new(POSConfig { inner })
    }

    /// Create a POS configuration with all options
    #[uniffi::constructor]
    pub fn with_options(
        descriptor: &WolletDescriptor,
        currency: &CurrencyCode,
        show_gear: Option<bool>,
        show_description: Option<bool>,
    ) -> Arc<POSConfig> {
        let mut inner =
            lwk_wollet::POSConfig::new(descriptor.as_ref().clone(), currency.as_ref().clone());

        if let Some(show_gear) = show_gear {
            inner = inner.with_show_gear(show_gear);
        }
        if let Some(show_description) = show_description {
            inner = inner.with_show_description(show_description);
        }

        Arc::new(POSConfig { inner })
    }

    /// Decode a POS configuration from a URL-safe base64 encoded string
    #[uniffi::constructor]
    pub fn decode(encoded: &str) -> Result<Arc<POSConfig>, LwkError> {
        let inner = lwk_wollet::POSConfig::decode(encoded).ok_or_else(|| LwkError::Generic {
            msg: "Invalid POS configuration encoding".to_string(),
        })?;
        Ok(Arc::new(POSConfig { inner }))
    }

    /// Encode the POS configuration to a URL-safe base64 string
    pub fn encode(&self) -> Result<String, LwkError> {
        self.inner.encode().map_err(|e| LwkError::Generic {
            msg: format!("Failed to encode POS configuration: {}", e),
        })
    }

    /// Get the wallet descriptor
    pub fn descriptor(&self) -> Arc<WolletDescriptor> {
        Arc::new(self.inner.descriptor.clone().into())
    }

    /// Get the currency code
    pub fn currency(&self) -> Arc<CurrencyCode> {
        Arc::new(self.inner.currency.clone().into())
    }

    /// Get whether to show the gear/settings button
    pub fn show_gear(&self) -> Option<bool> {
        self.inner.show_gear
    }

    /// Get whether to show the description/note field
    pub fn show_description(&self) -> Option<bool> {
        self.inner.show_description
    }
}
