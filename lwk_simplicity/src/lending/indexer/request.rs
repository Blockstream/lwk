use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
pub struct OfferFiltersRequest {
    pub status: Option<Vec<OfferStatus>>,
    pub collateral_asset: Option<String>,
    pub principal_asset: Option<String>,
    pub factory_id: Option<Uuid>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
    pub sort_by: Option<OfferSortBy>,
    pub sort_dir: Option<SortDir>,
}

impl OfferFiltersRequest {
    pub fn to_query_params(&self) -> Vec<(String, String)> {
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
