use boltz_client::boltz::SwapStatus;

use crate::Error;

/// Enum representing all possible swap status values from Boltz API updates
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Copy)]
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
    InvoiceFailedToPay,
    SwapCreated,
    TransactionDirect,
    InvoiceSettled,
    InvoiceExpired,
    SwapExpired,
}

pub(crate) trait SwapStateTrait {
    fn swap_state(&self) -> Result<SwapState, Error>;
}
impl SwapStateTrait for SwapStatus {
    fn swap_state(&self) -> Result<SwapState, Error> {
        self.status
            .parse::<SwapState>()
            .map_err(|_| Error::InvalidSwapState(self.status.clone()))
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
            SwapState::InvoiceFailedToPay => "invoice.failedToPay",
            SwapState::SwapCreated => "swap.created",
            SwapState::TransactionDirect => "transaction.direct",
            SwapState::InvoiceSettled => "invoice.settled",
            SwapState::InvoiceExpired => "invoice.expired",
            SwapState::SwapExpired => "swap.expired",
        };
        write!(f, "{}", s)
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
            "invoice.failedToPay" => Ok(SwapState::InvoiceFailedToPay),
            "swap.created" => Ok(SwapState::SwapCreated),
            "transaction.direct" => Ok(SwapState::TransactionDirect),
            "invoice.settled" => Ok(SwapState::InvoiceSettled),
            "invoice.expired" => Ok(SwapState::InvoiceExpired),
            "swap.expired" => Ok(SwapState::SwapExpired),
            _ => Err(format!("Unknown swap status: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::SwapState;

    #[test]
    fn test_swap_status_roundtrip() {
        let statuses = vec![
            SwapState::Initialized,
            SwapState::InvoiceSet,
            SwapState::TransactionMempool,
            SwapState::TransactionConfirmed,
            SwapState::InvoicePending,
            SwapState::InvoicePaid,
            SwapState::TransactionClaimPending,
            SwapState::TransactionClaimed,
            SwapState::TransactionLockupFailed,
            SwapState::InvoiceFailedToPay,
            SwapState::SwapCreated,
            SwapState::TransactionDirect,
            SwapState::InvoiceSettled,
        ];

        for status in statuses {
            // Test Display -> FromStr roundtrip
            let status_str = status.to_string();
            let parsed: SwapState = status_str.parse().unwrap();
            assert_eq!(status, parsed, "Failed roundtrip for status: {:?}", status);
        }
    }
}
