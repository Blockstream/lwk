mod sources;

use iso4217::CurrencyCode;
use std::time::Duration;

pub struct PricesFetcher {
    timeout: u8,
    client: reqwest::Client,
}

pub struct PricesFetcherBuilder {
    timeout: u8,
}

impl Default for PricesFetcherBuilder {
    fn default() -> Self {
        Self { timeout: 10 }
    }
}

impl PricesFetcherBuilder {
    pub fn timeout(mut self, timeout: u8) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> Result<PricesFetcher, Error> {
        let builder = reqwest::Client::builder();

        // Timeout is not supported in WASM
        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder.timeout(Duration::from_secs(self.timeout as u64));

        let client = builder.build().map_err(|e| Error::Http(e.to_string()))?;

        Ok(PricesFetcher {
            timeout: self.timeout,
            client,
        })
    }
}

/// Multiple exchange rates against BTC provided from various sources
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExchangeRates {
    pub results: Vec<ExchangeRate>,
    pub median: f64,
}

/// `rate` is the amount of `currency` needed to buy 1 BTC from `source`
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExchangeRate {
    pub rate: f64,
    #[serde(with = "currency_code_serde")]
    pub currency: CurrencyCode,
    pub source: String,
    pub timestamp: u64,
}

mod currency_code_serde {
    use iso4217::CurrencyCode;
    use serde::{Deserialize, Deserializer, Serializer};

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
        iso4217::alpha3(&s)
            .cloned()
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown currency: {s}")))
    }
}

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

const SUPPORTED_CURRENCIES: [&str; 3] = ["USD", "EUR", "CHF"];
const MIN_SOURCES: usize = 3;

impl PricesFetcher {
    pub fn new() -> Result<Self, Error> {
        Self::builder().build()
    }

    pub fn builder() -> PricesFetcherBuilder {
        PricesFetcherBuilder::default()
    }

    pub async fn rates(&self, currency: &str) -> Result<ExchangeRates, Error> {
        let currency_code = match iso4217::alpha3(currency) {
            Some(currency) => {
                if !SUPPORTED_CURRENCIES.contains(&currency.alpha3) {
                    return Err(Error::UnsupportedCurrency(currency.name.to_string()));
                }
                currency
            }
            None => return Err(Error::UnrecognizedCurrency(currency.to_string())),
        };

        // Fetch from all sources in parallel
        let fetchers = vec![
            sources::Source::Coinbase,
            sources::Source::Kraken,
            sources::Source::CoinGecko,
            sources::Source::Binance,
            sources::Source::CoinPaprika,
        ];

        let tasks: Vec<_> = fetchers
            .into_iter()
            .map(|fetcher| {
                let client = self.client.clone();
                let currency = currency_code.clone();
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
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap());

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
    use super::{Error, PricesFetcher};

    #[test]
    fn test_iso() {
        let currency = iso4217::alpha3("EUR").unwrap();
        assert_eq!(currency.name, "Euro");
    }

    #[tokio::test]
    async fn test_validation() {
        let fetcher = PricesFetcher::new().unwrap();

        let err = fetcher.rates("NOT_A_CURRENCY").await.unwrap_err();
        assert_eq!(
            err,
            Error::UnrecognizedCurrency("NOT_A_CURRENCY".to_string())
        );

        let err = fetcher.rates("JPY").await.unwrap_err();
        assert_eq!(err, Error::UnsupportedCurrency("Japanese yen".to_string()));
    }

    async fn test_fetch_rates(currency: &str) {
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
            assert_eq!(rate.currency.alpha3, currency);
            assert!(rate.timestamp > 0);
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
        test_fetch_rates("USD").await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_eur_rates() {
        test_fetch_rates("EUR").await;
    }

    #[tokio::test]
    #[ignore] // This test makes real API calls
    async fn test_fetch_chf_rates() {
        test_fetch_rates("CHF").await;
    }
}
