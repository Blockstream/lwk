//! Root Wallet ABI surface for provider-facing schema and runtime types.
//!
//! The checked-in provider product lives at [`WalletAbiProvider`]. It exposes:
//! - connect-time getters for signer identity and receive address
//! - typed request execution for tx-create and tx-evaluate flows
//! - provider discovery via [`WalletCapabilities`]
//! - method-level JSON dispatch for external transports
//!
//! The runtime engine itself remains [`WalletAbiRuntime`], which builds and evaluates requests
//! against request-scoped wallet dependencies.
//!
//! Typed usage:
//!
//! ```ignore
//! use lwk_simplicity::wallet_abi::WalletAbiProviderBuilder;
//!
//! async fn typed_flow() -> Result<(), lwk_simplicity::error::WalletAbiError> {
//!     let provider = WalletAbiProviderBuilder::new(
//!         signer_meta,
//!         session_factory,
//!         prevout_resolver,
//!         output_allocator,
//!         broadcaster,
//!         receive_address_provider,
//!     )
//!     .build();
//!
//!     let _xonly = provider.get_raw_signing_x_only_pubkey()?;
//!     let _address = provider.get_signer_receive_address()?;
//!     let _capabilities = provider.get_capabilities().await?;
//!     let _preview = provider.evaluate_request(evaluate_request).await?;
//!     let _response = provider.process_request(create_request).await?;
//!     Ok(())
//! }
//! ```
//!
//! JSON dispatch usage:
//!
//! ```ignore
//! use lwk_simplicity::wallet_abi::{
//!     WalletAbiProviderBuilder, WALLET_ABI_GET_CAPABILITIES_METHOD,
//! };
//!
//! async fn json_flow() -> Result<(), lwk_simplicity::error::WalletAbiError> {
//!     let provider = WalletAbiProviderBuilder::new(
//!         signer_meta,
//!         session_factory,
//!         prevout_resolver,
//!         output_allocator,
//!         broadcaster,
//!         receive_address_provider,
//!     )
//!     .build();
//!
//!     let _result = provider
//!         .dispatch_json(
//!             WALLET_ABI_GET_CAPABILITIES_METHOD,
//!             serde_json::Value::Null,
//!         )
//!         .await?;
//!     Ok(())
//! }
//! ```

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
