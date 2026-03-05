use crate::error::WalletAbiError;

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
    Unknown(String),
}

impl WalletAbiErrorCode {
    pub fn as_str(&self) -> &str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Serde => "serde",
            Self::ProgramError => "program_error",
            Self::Unknown(value) => value.as_str(),
        }
    }
}

impl From<&WalletAbiError> for WalletAbiErrorCode {
    fn from(e: &WalletAbiError) -> Self {
        match e {
            WalletAbiError::InvalidRequest(_) => Self::InvalidRequest,
            WalletAbiError::Serde(_) => Self::Serde,
            WalletAbiError::Program(_) => Self::ProgramError,
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

mod error_code_as_str {
    use super::WalletAbiErrorCode;
    use serde::{Deserialize, Deserializer, Serializer};

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
        let s = String::deserialize(d)?;
        let code = match s.as_str() {
            "invalid_request" => WalletAbiErrorCode::InvalidRequest,
            "serde" => WalletAbiErrorCode::Serde,
            "program_error" => WalletAbiErrorCode::ProgramError,
            _ => WalletAbiErrorCode::Unknown(s),
        };
        Ok(code)
    }
}
