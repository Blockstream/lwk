use std::{
    sync::{MutexGuard, PoisonError},
    time::SystemTimeError,
};

use serde::{Deserialize, Serialize};
use serde_cbor::Value;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Jade Error: {0}")]
    JadeError(ErrorDetails),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("SystemTime Error: {0}")]
    SystemTimeError(SystemTimeError),

    #[cfg(feature = "serial")]
    #[error("Serial Error: {0}")]
    SerialError(#[from] serialport::Error),

    #[error("No available ports")]
    NoAvailablePorts,

    #[error("Jade returned neither an error nor a result")]
    JadeNeitherErrorNorResult,

    #[error(transparent)]
    SerdeCbor(#[from] serde_cbor::Error),

    #[error(transparent)]
    Bip32(#[from] elements::bitcoin::bip32::Error),

    #[error("Mismatching network, jade was initialized with: {init} but the method params received {passed}")]
    MismatchingXpub {
        init: crate::Network,
        passed: crate::Network,
    },

    #[error("Poison error: {0}")]
    PoisonError(String),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error("Http request to {0} returned {1} instead of 200")]
    HttpStatus(String, u16),

    #[error("Jade authentication returned a response without a usable url")]
    NoUsableUrl,

    // TODO: this error description is useful in web UI when the PIN is wrong.
    // However, it may be related to other cases other than the wrong PIN
    #[error("Wrong Pin")]
    HandshakeFailed,

    #[error("Jade not initialized")]
    NotInitialized,

    #[error(transparent)]
    Pset(#[from] elements::pset::Error),

    #[error("Missing asset id in output {0}")]
    MissingAssetIdInOutput(usize),

    #[error("Missing blind asset proof in output {0}")]
    MissingBlindAssetProofInOutput(usize),

    #[error("Missing asset commitment in output {0}")]
    MissingAssetCommInOutput(usize),

    #[error("Missing blinding key in output {0}")]
    MissingBlindingKeyInOutput(usize),

    #[error("Missing amount in output {0}")]
    MissingAmountInOutput(usize),

    #[error("Missing amount commitment in output {0}")]
    MissingAmountCommInOutput(usize),

    #[error("Missing blind value proof in output {0}")]
    MissingBlindValueProofInOutput(usize),

    #[error("Missing witness utxo in input {0}")]
    MissingWitnessUtxoInInput(usize),

    #[error("Non confidential input {0}")]
    NonConfidentialInput(usize),

    #[error("Expecting bip 32 derivation for input {0}")]
    MissingBip32DerivInput(usize),

    #[error("Previous script pubkey is wsh but witness script is missing in input {0}")]
    MissingWitnessScript(usize),

    #[error("Unsupported spending script pubkey: {0}")]
    UnsupportedScriptPubkeyType(String),

    #[error("Only slip77 master blinding key are supported")]
    OnlySlip77Supported,

    #[error("Single key are not supported")]
    SingleKeyAreNotSupported,

    #[error("Unsupported descriptor type, only wsh is supported")]
    UnsupportedDescriptorType,

    #[error("Unsupported descriptor variant, only multi or sortedmulti are supported")]
    UnsupportedDescriptorVariant,

    #[error("Slip 77 master blinding keys must be 32 bytes")]
    Slip77MasterBlindingKeyInvalidSize,

    #[error(transparent)]
    HttpReqwest(#[from] reqwest::Error),

    #[error("{0}")]
    Generic(String),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorDetails {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl std::fmt::Display for ErrorDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error code: {} - message: {}", self.code, self.message)
    }
}

impl<T> From<PoisonError<MutexGuard<'_, T>>> for Error {
    fn from(e: PoisonError<MutexGuard<'_, T>>) -> Self {
        Error::PoisonError(e.to_string())
    }
}
