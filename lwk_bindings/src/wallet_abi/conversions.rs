use super::*;
use crate::wallet_abi::schema::roots::{WalletAbiErrorCode, WalletAbiStatus};

impl From<WalletAbiInputIssuanceKind> for abi::InputIssuanceKind {
    fn from(value: WalletAbiInputIssuanceKind) -> Self {
        match value {
            WalletAbiInputIssuanceKind::New => Self::New,
            WalletAbiInputIssuanceKind::Reissue => Self::Reissue,
        }
    }
}

impl From<abi::InputIssuanceKind> for WalletAbiInputIssuanceKind {
    fn from(value: abi::InputIssuanceKind) -> Self {
        match value {
            abi::InputIssuanceKind::New => Self::New,
            abi::InputIssuanceKind::Reissue => Self::Reissue,
        }
    }
}

impl From<&abi::WalletAbiErrorCode> for WalletAbiErrorCode {
    fn from(value: &abi::WalletAbiErrorCode) -> Self {
        match value {
            abi::WalletAbiErrorCode::InvalidRequest => Self::InvalidRequest,
            abi::WalletAbiErrorCode::Serde => Self::Serde,
            abi::WalletAbiErrorCode::ProgramError => Self::ProgramError,
            abi::WalletAbiErrorCode::Derivation => Self::Derivation,
            abi::WalletAbiErrorCode::TryFromInt => Self::TryFromInt,
            abi::WalletAbiErrorCode::Funding => Self::Funding,
            abi::WalletAbiErrorCode::InvalidSignerConfig => Self::InvalidSignerConfig,
            abi::WalletAbiErrorCode::InvalidResponse => Self::InvalidResponse,
            abi::WalletAbiErrorCode::Pset => Self::Pset,
            abi::WalletAbiErrorCode::PsetBlind => Self::PsetBlind,
            abi::WalletAbiErrorCode::AmountProofVerification => Self::AmountProofVerification,
            abi::WalletAbiErrorCode::InvalidFinalizationSteps => Self::InvalidFinalizationSteps,
            abi::WalletAbiErrorCode::Unknown(_) => Self::Unknown,
        }
    }
}

impl From<abi::tx_create::Status> for WalletAbiStatus {
    fn from(value: abi::tx_create::Status) -> Self {
        match value {
            abi::tx_create::Status::Ok => Self::Ok,
            abi::tx_create::Status::Error => Self::Error,
        }
    }
}
