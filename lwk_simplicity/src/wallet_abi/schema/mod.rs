//! Wallet ABI schema modules used by `wallet-abi-0.1`.
//!
//! Module map:
//! - [`runtime_params`]: transaction-construction request parameters.
//! - [`tx_create`]: request/response envelope types for transaction creation.
//! - [`types`]: shared envelope support types.
//! - [`values`]: Simplicity argument/witness serialization helpers.

pub mod runtime_deps;
pub mod runtime_params;
pub mod tx_create;
pub mod types;
pub mod values;

pub use runtime_deps::{
    KeyStoreMeta, WalletOutputRequest, WalletOutputTemplate, WalletProviderMeta,
    WalletRequestSession, WalletRuntimeDeps, WalletSessionFactory,
};
pub use runtime_params::{
    AmountFilter, AssetFilter, AssetVariant, BlinderVariant, FinalizerSpec, InputIssuance,
    InputIssuanceKind, InputSchema, InputUnblinding, InternalKeySource, LockFilter, LockVariant,
    OutputSchema, RuntimeParams, UTXOSource, WalletSourceFilter,
};
pub use tx_create::{
    generate_request_id, TransactionInfo, TxCreateArtifacts, TxCreateRequest, TxCreateResponse,
    TX_CREATE_ABI_VERSION,
};
pub use types::{ErrorInfo, WalletAbiErrorCode};
pub use values::{
    resolve_arguments, resolve_witness, serialize_arguments, serialize_witness, RuntimeSimfValue,
    RuntimeSimfWitness, SimfArguments, SimfWitness,
};
