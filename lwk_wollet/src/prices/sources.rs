use crate::asyncr::async_now;

use super::{Error, ExchangeRate};
use iso4217::CurrencyCode;
use serde::Deserialize;
use serde_json::Value;

// Response types for different APIs
#[derive(Deserialize)]
struct CoinbaseResponse {
    data: CoinbaseData,
}

#[derive(Deserialize)]
struct CoinbaseData {
    amount: String,
}

#[derive(Deserialize)]
struct BinanceResponse {
    price: String,
}

pub(super) enum Source {
    Coinbase,
    Kraken,
    CoinGecko,
    Binance,
    CoinPaprika,
}

impl Source {
    pub(super) async fn fetch(
        &self,
        client: &reqwest::Client,
        currency: &CurrencyCode,
    ) -> Result<ExchangeRate, Error> {
        let timestamp = async_now().await;

        match self {
            Source::Coinbase => {
                let url = format!(
                    "https://api.coinbase.com/v2/prices/BTC-{}/spot",
                    currency.alpha3
                );
                let response: CoinbaseResponse = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?;

                let rate = response
                    .data
                    .amount
                    .parse::<f64>()
                    .map_err(|e| Error::Http(format!("Parse error: {e}")))?;

                Ok(ExchangeRate {
                    rate,
                    currency: currency.clone(),
                    source: "Coinbase".to_string(),
                    timestamp,
                })
            }
            Source::Kraken => {
                let pair = format!("XBT{}", currency.alpha3);
                let url = format!("https://api.kraken.com/0/public/Ticker?pair={pair}");
                let response: Value = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?;

                // Kraken returns different keys based on currency (XXBTZUSD, XXBTZEUR, etc.)
                let result = response
                    .get("result")
                    .and_then(|r| r.as_object())
                    .and_then(|obj| obj.values().next())
                    .ok_or_else(|| Error::Http("Invalid Kraken response".to_string()))?;

                let rate = result
                    .get("c")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Http("Missing price in Kraken response".to_string()))?
                    .parse::<f64>()
                    .map_err(|e| Error::Http(format!("Parse error: {e}")))?;

                Ok(ExchangeRate {
                    rate,
                    currency: currency.clone(),
                    source: "Kraken".to_string(),
                    timestamp,
                })
            }
            Source::CoinGecko => {
                let currency_lower = currency.alpha3.to_lowercase();
                let url = format!("https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies={currency_lower}");
                let response: Value = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?;

                let rate = response
                    .get("bitcoin")
                    .and_then(|b| b.get(&currency_lower))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| Error::Http("Invalid CoinGecko response".to_string()))?;

                Ok(ExchangeRate {
                    rate,
                    currency: currency.clone(),
                    source: "CoinGecko".to_string(),
                    timestamp,
                })
            }
            Source::Binance => {
                let symbol = format!("BTC{}", currency.alpha3);
                let (base_url, source_name) = if currency.alpha3 == "USD" {
                    ("https://api.binance.us", "Binance US")
                } else {
                    ("https://api.binance.com", "Binance")
                };
                let url = format!("{base_url}/api/v3/ticker/price?symbol={symbol}");
                let response: BinanceResponse = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?;

                let rate = response
                    .price
                    .parse::<f64>()
                    .map_err(|e| Error::Http(format!("Parse error: {e}")))?;

                Ok(ExchangeRate {
                    rate,
                    currency: currency.clone(),
                    source: source_name.to_string(),
                    timestamp,
                })
            }
            Source::CoinPaprika => {
                let url = "https://api.coinpaprika.com/v1/tickers/btc-bitcoin";
                let response: Value = client
                    .get(url)
                    .send()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| Error::Http(e.to_string()))?;

                let rate = response
                    .get("quotes")
                    .and_then(|q| q.get(currency.alpha3))
                    .and_then(|c| c.get("price"))
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| Error::Http("Invalid CoinPaprika response".to_string()))?;

                Ok(ExchangeRate {
                    rate,
                    currency: currency.clone(),
                    source: "CoinPaprika".to_string(),
                    timestamp,
                })
            }
        }
    }
}
