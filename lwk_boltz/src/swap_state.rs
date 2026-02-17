use boltz_client::boltz::SwapStatus;
use serde::{Deserialize, Serialize};

use crate::Error;

/// Enum representing all possible swap status values from Boltz API updates
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum SwapState {
    Initialized, // This is the initial state when the swap is created
    InvoiceSet,
    TransactionMempool,
    TransactionConfirmed,
    InvoicePending,
    InvoicePaid,
    TransactionClaimPending,
    TransactionClaimed,
    TransactionLockupFailed,
    TransactionFailed,
    InvoiceFailedToPay,
    SwapCreated,
    TransactionDirect,
    InvoiceSettled,
    InvoiceExpired,
    SwapExpired,
    ServerTransactionMempool,
    ServerTransactionConfirmed,
    TransactionZeroconfRejected,
    TransactionRefunded,
}

pub trait SwapStateTrait {
    fn swap_state(&self) -> Result<SwapState, Error>;
}
impl SwapStateTrait for SwapStatus {
    fn swap_state(&self) -> Result<SwapState, Error> {
        self.status
            .parse::<SwapState>()
            .map_err(|_| Error::InvalidSwapState(self.status.clone()))
    }
}

impl SwapState {
    /// Returns true if this state is terminal (swap has completed or failed).
    ///
    /// Terminal states indicate the swap has reached a final outcome and
    /// no further `advance()` calls will change it.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SwapState::TransactionClaimed
                | SwapState::TransactionDirect
                | SwapState::InvoiceSettled
                | SwapState::InvoiceExpired
                | SwapState::SwapExpired
                | SwapState::TransactionFailed
        )
    }
}

impl std::fmt::Display for SwapState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SwapState::Initialized => "initialized",
            SwapState::InvoiceSet => "invoice.set",
            SwapState::TransactionMempool => "transaction.mempool",
            SwapState::TransactionConfirmed => "transaction.confirmed",
            SwapState::InvoicePending => "invoice.pending",
            SwapState::InvoicePaid => "invoice.paid",
            SwapState::TransactionClaimPending => "transaction.claim.pending",
            SwapState::TransactionClaimed => "transaction.claimed",
            SwapState::TransactionLockupFailed => "transaction.lockupFailed",
            SwapState::TransactionFailed => "transaction.failed",
            SwapState::InvoiceFailedToPay => "invoice.failedToPay",
            SwapState::SwapCreated => "swap.created",
            SwapState::TransactionDirect => "transaction.direct",
            SwapState::InvoiceSettled => "invoice.settled",
            SwapState::InvoiceExpired => "invoice.expired",
            SwapState::SwapExpired => "swap.expired",
            SwapState::ServerTransactionMempool => "transaction.server.mempool",
            SwapState::ServerTransactionConfirmed => "transaction.server.confirmed",
            SwapState::TransactionZeroconfRejected => "transaction.zeroconf.rejected",
            SwapState::TransactionRefunded => "transaction.refunded",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for SwapState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "initialized" => Ok(SwapState::Initialized),
            "invoice.set" => Ok(SwapState::InvoiceSet),
            "transaction.mempool" => Ok(SwapState::TransactionMempool),
            "transaction.confirmed" => Ok(SwapState::TransactionConfirmed),
            "invoice.pending" => Ok(SwapState::InvoicePending),
            "invoice.paid" => Ok(SwapState::InvoicePaid),
            "transaction.claim.pending" => Ok(SwapState::TransactionClaimPending),
            "transaction.claimed" => Ok(SwapState::TransactionClaimed),
            "transaction.lockupFailed" => Ok(SwapState::TransactionLockupFailed),
            "transaction.failed" => Ok(SwapState::TransactionFailed),
            "invoice.failedToPay" => Ok(SwapState::InvoiceFailedToPay),
            "swap.created" => Ok(SwapState::SwapCreated),
            "transaction.direct" => Ok(SwapState::TransactionDirect),
            "invoice.settled" => Ok(SwapState::InvoiceSettled),
            "invoice.expired" => Ok(SwapState::InvoiceExpired),
            "swap.expired" => Ok(SwapState::SwapExpired),
            "transaction.server.mempool" => Ok(SwapState::ServerTransactionMempool),
            "transaction.server.confirmed" => Ok(SwapState::ServerTransactionConfirmed),
            "transaction.zeroconf.rejected" => Ok(SwapState::TransactionZeroconfRejected),
            "transaction.refunded" => Ok(SwapState::TransactionRefunded),
            _ => Err(format!("Unknown swap status: {s}")),
        }
    }
}

impl Serialize for SwapState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SwapState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use crate::SwapState;

    fn all_swap_states() -> Vec<SwapState> {
        vec![
            SwapState::Initialized,
            SwapState::InvoiceSet,
            SwapState::TransactionMempool,
            SwapState::TransactionConfirmed,
            SwapState::InvoicePending,
            SwapState::InvoicePaid,
            SwapState::TransactionClaimPending,
            SwapState::TransactionClaimed,
            SwapState::TransactionLockupFailed,
            SwapState::TransactionFailed,
            SwapState::InvoiceFailedToPay,
            SwapState::SwapCreated,
            SwapState::TransactionDirect,
            SwapState::InvoiceSettled,
            SwapState::InvoiceExpired,
            SwapState::SwapExpired,
            SwapState::ServerTransactionMempool,
            SwapState::ServerTransactionConfirmed,
            SwapState::TransactionZeroconfRejected,
            SwapState::TransactionRefunded,
        ]
    }

    #[test]
    fn test_swap_status_roundtrip() {
        for status in all_swap_states() {
            // Test Display -> FromStr roundtrip
            let status_str = status.to_string();
            let parsed: SwapState = status_str.parse().unwrap();
            assert_eq!(status, parsed, "Failed roundtrip for status: {status:?}");
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        for status in all_swap_states() {
            // Test serde JSON roundtrip
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: SwapState = serde_json::from_str(&json).unwrap();
            assert_eq!(
                status, deserialized,
                "Failed serde roundtrip for status: {status:?}"
            );

            // Verify the JSON contains the dot-separated format (without quotes for simplicity)
            assert!(
                !json.contains("InvoiceSet"),
                "JSON should not contain PascalCase: {json}"
            );
            let expected = format!("\"{status}\"");
            assert_eq!(
                json, expected,
                "JSON should match Display format: expected {expected}, got {json}"
            );
        }
    }
}
