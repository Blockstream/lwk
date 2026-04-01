use crate::error::WalletAbiError;

use std::str::FromStr;

/// Canonical machine-readable and human-readable error payload for `wallet-abi-0.1`.
///
/// `ErrorInfo` appears in [`TxCreateResponse`](crate::wallet_abi::schema::tx_create::TxCreateResponse)
/// when `status` is `error`.
///
/// Contract guidance:
/// - `code` is a stable machine-facing category intended for branching/metrics.
/// - `message` is human-facing text and should not be used as a stable discriminator.
/// - `details` is optional JSON context and must be schema-validated by consumers
///   before trust-sensitive use.
///
/// # Serialization and compatibility
/// - `details` is omitted during serialization when absent (`None`).
/// - During deserialization, missing `details` and explicit `details: null` both map to `None`.
/// - Unknown `code` values are preserved as [`WalletAbiErrorCode::Unknown`].
/// - Consumers should still tolerate arbitrary JSON shapes in `details`.
///
/// # Security
/// - Treat `message` and `details` as untrusted data.
/// - Producers should avoid embedding secrets, credentials, or private keys in `details`.
/// - Consumers should sanitize/escape values before rendering in UI or HTML/Markdown contexts.
///
/// # UX guidance
/// - Use `code` for product logic and localization/error-map lookup.
/// - Use `message` as technical context, not as a stable product-facing key.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ErrorInfo {
    /// Machine-readable error classification key.
    #[serde(with = "error_code_as_str")]
    pub code: WalletAbiErrorCode,
    /// Human-readable technical diagnostic text.
    pub message: String,
    /// Optional structured producer-defined diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalletAbiErrorCode {
    InvalidRequest,
    Serde,
    ProgramError,
    Derivation,
    TryFromInt,
    Funding,
    InvalidSignerConfig,
    InvalidResponse,
    Pset,
    PsetBlind,
    AmountProofVerification,
    InvalidFinalizationSteps,
    Unknown(String),
}

impl WalletAbiErrorCode {
    pub fn as_str(&self) -> &str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Serde => "serde",
            Self::ProgramError => "program_error",
            Self::Derivation => "derivation",
            Self::TryFromInt => "try_from_int",
            Self::Funding => "funding",
            Self::InvalidSignerConfig => "invalid_signer_config",
            Self::InvalidResponse => "invalid_response",
            Self::Pset => "pset",
            Self::PsetBlind => "pset_blind",
            Self::AmountProofVerification => "amount_proof_verification",
            Self::InvalidFinalizationSteps => "invalid_finalization_steps",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl FromStr for WalletAbiErrorCode {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "invalid_request" => Self::InvalidRequest,
            "serde" => Self::Serde,
            "program_error" => Self::ProgramError,
            "derivation" => Self::Derivation,
            "try_from_int" => Self::TryFromInt,
            "funding" => Self::Funding,
            "invalid_signer_config" => Self::InvalidSignerConfig,
            "invalid_response" => Self::InvalidResponse,
            "pset" => Self::Pset,
            "pset_blind" => Self::PsetBlind,
            "amount_proof_verification" => Self::AmountProofVerification,
            "invalid_finalization_steps" => Self::InvalidFinalizationSteps,
            _ => Self::Unknown(s.to_owned()),
        })
    }
}

impl From<&WalletAbiError> for WalletAbiErrorCode {
    fn from(e: &WalletAbiError) -> Self {
        match e {
            WalletAbiError::InvalidRequest(_) => Self::InvalidRequest,
            WalletAbiError::Serde(_) => Self::Serde,
            WalletAbiError::Program(_) => Self::ProgramError,
            WalletAbiError::Derivation(_) => Self::Derivation,
            WalletAbiError::TryFromInt(_) => Self::TryFromInt,
            WalletAbiError::Funding(_) => Self::Funding,
            WalletAbiError::InvalidSignerConfig(_) => Self::InvalidSignerConfig,
            WalletAbiError::InvalidResponse(_) => Self::InvalidResponse,
            WalletAbiError::Pset(_) => Self::Pset,
            WalletAbiError::PsetBlind(_) => Self::PsetBlind,
            WalletAbiError::AmountProofVerification(_) => Self::AmountProofVerification,
            WalletAbiError::InvalidFinalizationSteps(_) => Self::InvalidFinalizationSteps,
        }
    }
}

impl From<&WalletAbiError> for ErrorInfo {
    fn from(error: &WalletAbiError) -> Self {
        Self {
            code: error.into(),
            message: error.to_string(),
            details: None,
        }
    }
}

impl From<WalletAbiError> for ErrorInfo {
    fn from(error: WalletAbiError) -> Self {
        (&error).into()
    }
}

impl ErrorInfo {
    /// Build an error payload from a canonical error code string and optional JSON details.
    pub fn from_code_and_json(
        code: &str,
        message: impl Into<String>,
        details_json: Option<&str>,
    ) -> Result<Self, WalletAbiError> {
        Ok(Self {
            code: match WalletAbiErrorCode::from_str(code) {
                Ok(code) => code,
                Err(err) => match err {},
            },
            message: message.into(),
            details: details_json
                .map(serde_json::from_str)
                .transpose()
                .map_err(WalletAbiError::from)?,
        })
    }

    /// Serialize the optional `details` payload as canonical JSON.
    pub fn details_json(&self) -> Result<Option<String>, WalletAbiError> {
        self.details
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(WalletAbiError::from)
    }
}

mod error_code_as_str {
    use super::WalletAbiErrorCode;

    use serde::{Deserialize, Deserializer, Serializer};

    use std::str::FromStr;

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S>(code: &WalletAbiErrorCode, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(code.as_str())
    }

    pub fn deserialize<'de, D>(d: D) -> Result<WalletAbiErrorCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match WalletAbiErrorCode::from_str(&String::deserialize(d)?) {
                Ok(code) => code,
                Err(err) => match err {},
            },
        )
    }
}
