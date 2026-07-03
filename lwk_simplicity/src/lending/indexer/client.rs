//! Indexer client implementation for indexer in `simplicity-lending`.
//! <https://github.com/BlockstreamResearch/simplicity-lending/tree/main/crates/indexer#api-reference>

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::lending::LendingError;

#[derive(Clone, Debug)]
pub struct IndexerClient {
    base_url: String,
    client: reqwest::Client,
}

impl IndexerClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, LendingError> {
        let mut url: String = base_url.into();
        while url.ends_with('/') {
            url.pop();
        }
        Ok(Self {
            base_url: url,
            client: reqwest::Client::builder()
                .build()
                .map_err(|err| LendingError::Config(err.to_string()))?,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn list_offers(
        &self,
        filters: &OfferFilters,
    ) -> Result<OfferListResponse, ClientError> {
        let params = filters.to_query_params();
        let resp = self
            .client
            .get(format!("{}/offers", self.base_url))
            .query(&params)
            .send()
            .await?;
        deserialize_response(resp).await
    }

    pub async fn get_factories_by_script(
        &self,
        script_pubkey: &str,
    ) -> Result<Vec<FactoryDetailsResponse>, ClientError> {
        let resp = self
            .client
            .get(format!("{}/factories/by-script", self.base_url))
            .query(&[("script_pubkey", script_pubkey)])
            .send()
            .await?;
        deserialize_response(resp).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferStatus {
    Pending,
    Active,
    Repaid,
    Liquidated,
    Cancelled,
    Claimed,
}

impl std::fmt::Display for OfferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OfferStatus::Pending => f.write_str("pending"),
            OfferStatus::Active => f.write_str("active"),
            OfferStatus::Repaid => f.write_str("repaid"),
            OfferStatus::Liquidated => f.write_str("liquidated"),
            OfferStatus::Cancelled => f.write_str("cancelled"),
            OfferStatus::Claimed => f.write_str("claimed"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    Desc,
    Asc,
}

impl std::fmt::Display for SortDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortDir::Desc => f.write_str("desc"),
            SortDir::Asc => f.write_str("asc"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferSortBy {
    CreatedAtHeight,
    CollateralAmount,
    PrincipalAmount,
    InterestRate,
    LoanExpirationHeight,
}

impl std::fmt::Display for OfferSortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OfferSortBy::CreatedAtHeight => f.write_str("created_at_height"),
            OfferSortBy::CollateralAmount => f.write_str("collateral_amount"),
            OfferSortBy::PrincipalAmount => f.write_str("principal_amount"),
            OfferSortBy::InterestRate => f.write_str("interest_rate"),
            OfferSortBy::LoanExpirationHeight => f.write_str("loan_expiration_height"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OfferFilters {
    pub status: Option<Vec<OfferStatus>>,
    pub collateral_asset: Option<String>,
    pub principal_asset: Option<String>,
    pub factory_id: Option<Uuid>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub sort_by: Option<OfferSortBy>,
    pub sort_dir: Option<SortDir>,
}

impl OfferFilters {
    fn to_query_params(&self) -> Vec<(String, String)> {
        let mut params = Vec::new();
        if let Some(ref statuses) = self.status {
            if !statuses.is_empty() {
                let joined = statuses
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                params.push(("status".to_string(), joined));
            }
        }
        if let Some(ref v) = self.collateral_asset {
            params.push(("collateral_asset".to_string(), v.clone()));
        }
        if let Some(ref v) = self.principal_asset {
            params.push(("principal_asset".to_string(), v.clone()));
        }
        if let Some(ref v) = self.factory_id {
            params.push(("factory_id".to_string(), v.to_string()));
        }
        if let Some(v) = self.limit {
            params.push(("limit".to_string(), v.to_string()));
        }
        if let Some(v) = self.offset {
            params.push(("offset".to_string(), v.to_string()));
        }
        if let Some(ref v) = self.sort_by {
            params.push(("sort_by".to_string(), v.to_string()));
        }
        if let Some(ref v) = self.sort_dir {
            params.push(("sort_dir".to_string(), v.to_string()));
        }
        params
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OfferListItemShort {
    pub id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OfferListResponse {
    pub items: Vec<OfferListItemShort>,
    pub total: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactoryStatus {
    Active,
    Removed,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactoryProgramUtxoDto {
    pub txid: String,
    pub vout: u32,
    pub created_at_height: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactoryAuthUtxoDto {
    pub txid: String,
    pub vout: u32,
    pub script_pubkey: String,
    pub created_at_height: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactoryDetailsResponse {
    pub id: Uuid,
    pub factory_asset_id: String,
    pub program_script_pubkey: String,
    pub status: FactoryStatus,
    pub issuing_utxos_count: u16,
    pub reissuance_flags: u64,
    pub created_at_height: u64,
    pub created_at_txid: String,
    pub auth_utxo: Option<FactoryAuthUtxoDto>,
    pub program_utxo: Option<FactoryProgramUtxoDto>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    error: ApiErrorMessage,
}

#[derive(Debug, Deserialize)]
struct ApiErrorMessage {
    code: String,
    message: String,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error ({code}): {message}")]
    Api {
        code: String,
        message: String,
        status: StatusCode,
    },

    #[error("Unexpected response {status}: {body}")]
    Unexpected { status: StatusCode, body: String },

    #[error("Failed to deserialize response: {0}")]
    Deserialize(String),
}

async fn deserialize_response<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
) -> Result<T, ClientError> {
    let status = resp.status();
    if status.is_success() {
        resp.json()
            .await
            .map_err(|e| ClientError::Deserialize(e.to_string()))
    } else {
        let body = resp.text().await.unwrap_or_default();
        if let Ok(api_err) = serde_json::from_str::<ApiErrorBody>(&body) {
            Err(ClientError::Api {
                code: api_err.error.code,
                message: api_err.error.message,
                status,
            })
        } else {
            Err(ClientError::Unexpected { status, body })
        }
    }
}
