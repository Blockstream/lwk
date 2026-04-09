//! Root Wallet ABI surface for provider-facing schema and runtime types.

mod provider;
pub mod schema;
pub mod tx_resolution;

pub use provider::{
    GET_RAW_SIGNING_X_ONLY_PUBKEY_METHOD, GET_SIGNER_RECEIVE_ADDRESS_METHOD,
    WALLET_ABI_EVALUATE_REQUEST_METHOD, WALLET_ABI_GET_CAPABILITIES_METHOD,
    WALLET_ABI_PROCESS_REQUEST_METHOD, WalletAbiProvider, WalletAbiProviderBuilder,
};
pub use schema::{
    deserialize_arguments, deserialize_witness, generate_request_id, resolve_arguments,
    resolve_witness, serialize_arguments, serialize_witness, AmountFilter, AssetFilter,
    AssetVariant, BlinderVariant, ErrorInfo, FinalizerSpec, InputIssuance, InputIssuanceKind,
    InputSchema, InputUnblinding, InternalKeySource, KeyStoreMeta, LockFilter, LockVariant,
    PreviewAssetDelta, PreviewOutput, PreviewOutputKind, RequestPreview, RuntimeParams,
    RuntimeSimfValue, RuntimeSimfWitness, SimfArguments, SimfWitness, TransactionInfo,
    TxCreateArtifacts, TxCreateRequest, TxCreateResponse, TxEvaluateRequest, TxEvaluateResponse,
    UTXOSource, WalletAbiErrorCode, WalletBroadcaster, WalletCapabilities, WalletOutputAllocator,
    WalletOutputRequest, WalletOutputTemplate, WalletPrevoutResolver, WalletProviderMeta,
    WalletReceiveAddressProvider, WalletRequestSession, WalletRuntimeDeps, WalletSessionFactory,
    WalletSourceFilter, TX_CREATE_ABI_VERSION,
};
pub use tx_resolution::runtime::Runtime as WalletAbiRuntime;
