//! # Prices
//!
//! This module provides a fetcher for exchange rates of fiat currencies against BTC.

pub use crate::prices::currency_code::CurrencyCode;

mod codes;
mod currency_code;
mod sources;

/// A fetcher for exchange rates
pub struct PricesFetcher {
    client: reqwest::Client,
}

/// A builder for the [`PricesFetcher`]
pub struct PricesFetcherBuilder {
    timeout: u8,
}

impl Default for PricesFetcherBuilder {
    fn default() -> Self {
        Self { timeout: 10 }
    }
}

impl PricesFetcherBuilder {
    /// Set the timeout for HTTP requests in seconds
    pub fn timeout(mut self, timeout: u8) -> Self {
        self.timeout = timeout;
        self
    }

    /// Build the PricesFetcher
    pub fn build(self) -> Result<PricesFetcher, Error> {
        let builder = reqwest::Client::builder();

        // Timeout is not supported in WASM
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(std::time::Duration::from_secs(self.timeout as u64));

        let client = builder.build().map_err(|e| Error::Http(e.to_string()))?;

        Ok(PricesFetcher { client })
    }
}

/// Multiple exchange rates against BTC provided from various sources
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExchangeRates {
    /// The list of exchange rates
    pub results: Vec<ExchangeRate>,

    /// The median of the exchange rates
    ///
    /// The median is the middle value of the exchange rates when sorted by rate.
    /// It's useful cause it's robust by not beingaffected by the outliers.
    pub median: f64,
}

/// `rate` is the amount of `currency` needed to buy 1 BTC from `source`
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExchangeRate {
    /// The rate of the currency in relation to BTC
    pub rate: f64,

    /// The currency code
    #[serde(with = "currency_code_serde")]
    pub currency: CurrencyCode,

    /// The source of the rate
    pub source: String,

    /// The fetch timestamp of the rate
    pub timestamp: u64,
}

mod currency_code_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::prices::currency_code::{self, CurrencyCode};

    pub fn serialize<S>(currency: &CurrencyCode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(currency.alpha3)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<CurrencyCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        currency_code::alpha3(&s)
            .cloned()
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown currency: {s}")))
    }
}

#[allow(missing_docs)]
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Unrecognized currency: {0}")]
    UnrecognizedCurrency(String),

    #[error("Unsupported currency: {0}")]
    UnsupportedCurrency(String),

    #[error("Not enough sources available (got {0}, need at least 3)")]
    NotEnoughSources(usize),

    #[error("HTTP error: {0}")]
    Http(String),
}

const SUPPORTED_CURRENCIES: [&str; 10] = [
    "USD", "EUR", "CHF", "GBP", "INR", "CAD", "ARS", "AUD", "BRL", "CNY",
];
const MIN_SOURCES: usize = 3;

impl PricesFetcher {
    /// Create a new PricesFetcher with default settings
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    /// Get a builder for configuring the PricesFetcher
    pub fn builder() -> PricesFetcherBuilder {
        PricesFetcherBuilder::default()
    }

    /// Fetch exchange rates for the given currency
    ///
    /// Multiple sources are used to fetch the rates
    pub async fn rates(&self, currency: &CurrencyCode) -> Result<ExchangeRates, Error> {
        if !SUPPORTED_CURRENCIES.contains(&currency.alpha3) {
            return Err(Error::UnsupportedCurrency(currency.name.to_string()));
        }

        // Fetch from all sources in parallel
        let fetchers = vec![
            sources::Source::Coinbase,
            sources::Source::Kraken,
            sources::Source::CoinGecko,
            sources::Source::Binance,
            sources::Source::CoinPaprika,
            sources::Source::BlockchainInfo,
        ];

        let tasks: Vec<_> = fetchers
            .into_iter()
            .map(|fetcher| {
                let client = self.client.clone();
                let currency = currency.clone();
                async move { fetcher.fetch(&client, &currency).await }
            })
            .collect();

        // Wait for all tasks and collect successful results
        let results = futures::future::join_all(tasks).await;
        let rates: Vec<ExchangeRate> = results.into_iter().filter_map(|r| r.ok()).collect();

        if rates.len() < MIN_SOURCES {
            return Err(Error::NotEnoughSources(rates.len()));
        }

        // Calculate median
        let mut prices: Vec<f64> = rates.iter().map(|r| r.rate).collect();
        prices.sort_by(|a, b| a.total_cmp(b));

        let median = if prices.len() % 2 == 0 {
            let mid = prices.len() / 2;
            (prices[mid - 1] + prices[mid]) / 2.0
        } else {
            prices[prices.len() / 2]
        };

        Ok(ExchangeRates {
            results: rates,
            median,
        })
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::CurrencyCode;

    use super::{currency_code, Error, PricesFetcher};

    #[test]
    fn test_iso() {
        let currency = currency_code::alpha3("EUR").unwrap();
        assert_eq!(currency.name, "Euro");
    }

    #[tokio::test]
    async fn test_validation() {
        let fetcher = PricesFetcher::new().unwrap();
        let err = CurrencyCode::from_str("NOT_A_CURRENCY").unwrap_err();
        assert_eq!(
            err,
            Error::UnrecognizedCurrency("NOT_A_CURRENCY".to_string())
        );
        let currency = CurrencyCode::from_str("JPY").unwrap();
        assert_eq!(currency.name, "Japanese yen");

        let err = fetcher.rates(&currency).await.unwrap_err();
        assert_eq!(err, Error::UnsupportedCurrency("Japanese yen".to_string()));
    }

    async fn test_fetch_rates(currency: &CurrencyCode) {
        let fetcher = PricesFetcher::new().unwrap();
        let rates = fetcher.rates(currency).await.unwrap();

        assert!(rates.results.len() >= 3, "Should have at least 3 sources");
        assert!(rates.median > 0.0, "Median price should be positive");

        // Check that all rates are reasonable (within 10% of median)
        for rate in &rates.results {
            let diff_pct = ((rate.rate - rates.median) / rates.median).abs() * 100.0;
            assert!(
                diff_pct < 10.0,
                "Rate from {} differs too much from median: {}% (rate: {}, median: {})",
                rate.source,
                diff_pct,
                rate.rate,
                rates.median
            );
            assert_eq!(rate.currency, *currency);
            assert!(rate.timestamp > 0, "{rate:?}");
        }

        println!(
            "Fetched {} rates for {}, median: {}",
            rates.results.len(),
            currency,
            rates.median
        );
        for rate in &rates.results {
            println!("  {}: {}", rate.source, rate.rate);
        }
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_usd_rates() {
        test_fetch_rates(&CurrencyCode::from_str("USD").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_eur_rates() {
        test_fetch_rates(&CurrencyCode::from_str("EUR").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_chf_rates() {
        test_fetch_rates(&CurrencyCode::from_str("CHF").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_gbp_rates() {
        test_fetch_rates(&CurrencyCode::from_str("GBP").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_inr_rates() {
        test_fetch_rates(&CurrencyCode::from_str("INR").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_cad_rates() {
        test_fetch_rates(&CurrencyCode::from_str("CAD").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_ars_rates() {
        test_fetch_rates(&CurrencyCode::from_str("ARS").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_aud_rates() {
        test_fetch_rates(&CurrencyCode::from_str("AUD").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_brl_rates() {
        test_fetch_rates(&CurrencyCode::from_str("BRL").unwrap()).await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_cny_rates() {
        test_fetch_rates(&CurrencyCode::from_str("CNY").unwrap()).await;
    }
}
