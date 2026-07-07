use serde::{Deserialize, Serialize};

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
