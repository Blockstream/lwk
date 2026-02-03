use wasm_bindgen::prelude::*;

use crate::{Error, WolletDescriptor};

use super::prices::CurrencyCode;

/// POS (Point of Sale) configuration for encoding/decoding
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct PosConfig {
    inner: lwk_wollet::PosConfig,
}

impl From<lwk_wollet::PosConfig> for PosConfig {
    fn from(inner: lwk_wollet::PosConfig) -> Self {
        Self { inner }
    }
}

impl From<PosConfig> for lwk_wollet::PosConfig {
    fn from(value: PosConfig) -> Self {
        value.inner
    }
}

impl From<&PosConfig> for lwk_wollet::PosConfig {
    fn from(value: &PosConfig) -> Self {
        value.inner.clone()
    }
}

#[wasm_bindgen]
impl PosConfig {
    /// Create a new POS configuration
    #[wasm_bindgen(constructor)]
    pub fn new(descriptor: &WolletDescriptor, currency: &CurrencyCode) -> PosConfig {
        let inner =
            lwk_wollet::PosConfig::new(descriptor.as_ref().clone(), currency.as_ref().clone());
        PosConfig { inner }
    }

    /// Create a POS configuration with all options
    #[wasm_bindgen(js_name = withOptions)]
    pub fn with_options(
        descriptor: &WolletDescriptor,
        currency: &CurrencyCode,
        show_gear: Option<bool>,
        show_description: Option<bool>,
    ) -> PosConfig {
        let mut inner =
            lwk_wollet::PosConfig::new(descriptor.as_ref().clone(), currency.as_ref().clone());

        if let Some(show_gear) = show_gear {
            inner = inner.with_show_gear(show_gear);
        }
        if let Some(show_description) = show_description {
            inner = inner.with_show_description(show_description);
        }

        PosConfig { inner }
    }

    /// Decode a POS configuration from a URL-safe base64 encoded string
    #[wasm_bindgen(js_name = decode)]
    pub fn decode(encoded: &str) -> Result<PosConfig, Error> {
        let inner = lwk_wollet::PosConfig::decode(encoded)
            .ok_or_else(|| Error::Generic("Invalid POS configuration encoding".to_string()))?;
        Ok(PosConfig { inner })
    }

    /// Encode the POS configuration to a URL-safe base64 string
    #[wasm_bindgen(js_name = encode)]
    pub fn encode(&self) -> Result<String, Error> {
        self.inner
            .encode()
            .map_err(|e| Error::Generic(format!("Failed to encode POS configuration: {e}")))
    }

    /// Get the wallet descriptor
    #[wasm_bindgen(getter = descriptor)]
    pub fn descriptor(&self) -> WolletDescriptor {
        self.inner.descriptor.clone().into()
    }

    /// Get the currency code
    #[wasm_bindgen(getter = currency)]
    pub fn currency(&self) -> CurrencyCode {
        self.inner.currency.clone().into()
    }

    /// Get whether to show the gear/settings button
    #[wasm_bindgen(getter = showGear)]
    pub fn show_gear(&self) -> Option<bool> {
        self.inner.show_gear
    }

    /// Get whether to show the description/note field
    #[wasm_bindgen(getter = showDescription)]
    pub fn show_description(&self) -> Option<bool> {
        self.inner.show_description
    }

    /// Return a string representation of the POS configuration
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!(
            "PosConfig(descriptor: {}, currency: {})",
            self.descriptor(),
            self.currency().alpha3()
        )
    }
}
