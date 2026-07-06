//! Client implementation for lending indexer in `simplicity-lending`.
//! <https://github.com/BlockstreamResearch/simplicity-lending/tree/main/crates/indexer#api-reference>

use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use super::request::OfferFiltersRequest;
use crate::lending::LendingError;

#[derive(Clone, Debug)]
pub struct IndexerClientBuilder {
    base_url: String,
}

impl IndexerClientBuilder {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub fn build(self) -> Result<IndexerClient, LendingError> {
        let mut url: String = self.base_url;
        while url.ends_with('/') {
            url.pop();
        }

        Ok(IndexerClient {
            base_url: url,
            client: reqwest::Client::builder()
                .build()
                .map_err(|err| LendingError::Config(err.to_string()))?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct IndexerClient {
    base_url: String,
    client: reqwest::Client,
}

impl IndexerClient {
    pub fn builder(base_url: impl Into<String>) -> IndexerClientBuilder {
        IndexerClientBuilder::new(base_url)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn list_offers(
        &self,
        filters: &OfferFiltersRequest,
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

    /// Get factories for the given script pubkey.
    ///
    /// A factory is a UTXO which allows the owner to create offers.
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

#[derive(Debug, Clone, Deserialize)]
pub struct OfferListItem {
    pub id: Uuid,
    pub issuance_factory_id: Uuid,
    pub status: OfferStatus,
    pub collateral_asset: String,
    pub principal_asset: String,
    pub collateral_amount: String,
    pub principal_amount: String,
    pub interest_rate: u32,
    pub loan_expiration_height: u32,
    pub created_at_height: u64,
    pub created_at_txid: String,
    pub participants: Vec<ParticipantShort>,
    pub borrower_principal_utxo: Option<OfferUtxoOutpointShort>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParticipantType {
    Borrower,
    Lender,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParticipantShort {
    pub participant_type: ParticipantType,
    pub script_pubkey: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OfferUtxoOutpointShort {
    pub txid: String,
    pub vout: u32,
}
#[derive(Debug, Clone, Deserialize)]
pub struct OfferListResponse {
    pub items: Vec<OfferListItem>,
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
