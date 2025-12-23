use std::sync::Arc;

/// Currency code as defined by ISO 4217
#[derive(uniffi::Object, PartialEq, Eq, Debug, Clone)]
#[uniffi::export(Display, Eq)]
pub struct CurrencyCode {
    pub(crate) inner: lwk_wollet::CurrencyCode,
}

impl std::fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl AsRef<lwk_wollet::CurrencyCode> for CurrencyCode {
    fn as_ref(&self) -> &lwk_wollet::CurrencyCode {
        &self.inner
    }
}

impl From<lwk_wollet::CurrencyCode> for CurrencyCode {
    fn from(inner: lwk_wollet::CurrencyCode) -> Self {
        Self { inner }
    }
}

impl From<CurrencyCode> for lwk_wollet::CurrencyCode {
    fn from(value: CurrencyCode) -> Self {
        value.inner
    }
}

impl From<&CurrencyCode> for lwk_wollet::CurrencyCode {
    fn from(value: &CurrencyCode) -> Self {
        value.inner.clone()
    }
}

#[uniffi::export]
impl CurrencyCode {
    /// Create a CurrencyCode from an alpha3 code (e.g., "USD", "EUR")
    #[uniffi::constructor]
    pub fn new(alpha3: &str) -> Result<Arc<CurrencyCode>, crate::LwkError> {
        let inner = alpha3.parse().map_err(|_| crate::LwkError::Generic {
            msg: format!("Invalid currency code: {}", alpha3),
        })?;
        Ok(Arc::new(CurrencyCode { inner }))
    }

    /// Get the alpha3 code (e.g., "USD")
    pub fn alpha3(&self) -> String {
        self.inner.alpha3.to_string()
    }

    /// Get the currency name (e.g., "US Dollar")
    pub fn name(&self) -> String {
        self.inner.name.to_string()
    }

    /// Get the number of decimals for this currency
    pub fn exp(&self) -> i8 {
        self.inner.exp
    }
}
