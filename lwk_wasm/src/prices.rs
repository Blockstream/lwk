use std::str::FromStr;

use wasm_bindgen::prelude::*;

use crate::Error;

/// Wrapper over [`lwk_wollet::PricesFetcher`]
#[wasm_bindgen]
pub struct PricesFetcher {
    inner: lwk_wollet::PricesFetcher,
}

/// Wrapper over [`lwk_wollet::PricesFetcherBuilder`]
#[wasm_bindgen]
pub struct PricesFetcherBuilder {
    inner: lwk_wollet::PricesFetcherBuilder,
}

impl From<lwk_wollet::PricesFetcherBuilder> for PricesFetcherBuilder {
    fn from(inner: lwk_wollet::PricesFetcherBuilder) -> Self {
        Self { inner }
    }
}

impl From<PricesFetcherBuilder> for lwk_wollet::PricesFetcherBuilder {
    fn from(builder: PricesFetcherBuilder) -> Self {
        builder.inner
    }
}

#[wasm_bindgen]
impl PricesFetcher {
    /// Create a new PricesFetcher with default settings
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<PricesFetcher, Error> {
        let inner = lwk_wollet::PricesFetcher::new()?;
        Ok(PricesFetcher { inner })
    }

    /// Fetch exchange rates for the given currency (e.g., "USD", "EUR", "CHF")
    ///
    /// Returns an ExchangeRates object containing rates from multiple sources and the median
    pub async fn rates(&self, currency: &CurrencyCode) -> Result<ExchangeRates, Error> {
        let inner = self.inner.rates(currency.as_ref()).await?;
        Ok(ExchangeRates { inner })
    }
}

#[wasm_bindgen]
pub struct CurrencyCode {
    inner: lwk_wollet::CurrencyCode,
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

#[wasm_bindgen]
impl CurrencyCode {
    #[wasm_bindgen(constructor)]
    pub fn new(code: &str) -> Result<CurrencyCode, Error> {
        let inner = lwk_wollet::CurrencyCode::from_str(code)?;
        Ok(CurrencyCode { inner })
    }

    pub fn name(&self) -> String {
        self.inner.name.to_string()
    }

    pub fn alpha3(&self) -> String {
        self.inner.alpha3.to_string()
    }

    pub fn exp(&self) -> i8 {
        self.inner.exp
    }
}

/// Multiple exchange rates against BTC provided from various sources
#[wasm_bindgen]
pub struct ExchangeRates {
    inner: lwk_wollet::ExchangeRates,
}

#[wasm_bindgen]
impl ExchangeRates {
    /// Get the median exchange rate
    pub fn median(&self) -> f64 {
        self.inner.median
    }

    /// Get the individual exchange rates as a JSON array
    ///
    /// Each rate contains: rate, currency, source, and timestamp
    pub fn results(&self) -> Result<JsValue, Error> {
        Ok(serde_wasm_bindgen::to_value(&self.inner.results)?)
    }

    /// Get the number of sources that provided rates
    #[wasm_bindgen(js_name = resultsCount)]
    pub fn results_count(&self) -> usize {
        self.inner.results.len()
    }

    /// Serialize the entire response to JSON string
    pub fn serialize(&self) -> Result<String, Error> {
        Ok(serde_json::to_string(&self.inner)?)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_prices_fetcher() {
        let fetcher = PricesFetcher::new().unwrap();
        let usd = CurrencyCode::new("USD").unwrap();
        let rates = fetcher.rates(&usd).await.unwrap();

        assert!(rates.median() > 0.0);
        assert!(rates.results_count() >= 3);

        let json = rates.serialize().unwrap();
        assert!(json.contains("median"));
    }

    #[wasm_bindgen_test]
    async fn test_prices_fetcher_builder() {
        let fetcher = PricesFetcher::new().unwrap();
        let eur = CurrencyCode::new("EUR").unwrap();
        let rates = fetcher.rates(&eur).await.unwrap();

        assert!(rates.median() > 0.0);
        assert!(rates.results_count() >= 3);
    }

    #[wasm_bindgen_test]
    async fn test_invalid_currency() {
        let err = CurrencyCode::new("INVALID");
        assert!(err.is_err());
    }

    #[wasm_bindgen_test]
    async fn test_unsupported_currency() {
        let fetcher = PricesFetcher::new().unwrap();
        // JPY is a valid currency code but not supported by price sources
        let jpy = CurrencyCode::new("JPY").unwrap();
        let err = fetcher.rates(&jpy).await;
        assert!(err.is_err());
    }
}
