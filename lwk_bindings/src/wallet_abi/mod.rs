use std::str::FromStr;
use std::sync::Arc;

use crate::simplicity::{SimplicityArguments, SimplicityWitnessValues};
use crate::types::{AssetId, LockTime, PublicKey, SecretKey, TxSequence, XOnlyPublicKey};
use crate::{LwkError, Network, OutPoint, Script, Txid};

use lwk_simplicity::taproot_pubkey_gen::TaprootPubkeyGen;
use lwk_simplicity::wallet_abi::schema as abi;

mod conversions;
mod bip32;
mod output_request;
mod output_template;
mod request_session;
mod signer_context;
mod schema;

pub use bip32::{
    wallet_abi_bip32_derivation_pair_from_signer, WalletAbiBip32DerivationPair,
};
pub use output_request::{WalletAbiWalletOutputRequest, WalletAbiWalletOutputRole};
pub use output_template::{wallet_abi_output_template_from_address, WalletAbiWalletOutputTemplate};
pub use request_session::WalletAbiRequestSession;
pub use signer_context::WalletAbiSignerContext;
pub use schema::capabilities::WalletAbiCapabilities;
pub use schema::evaluate::{WalletAbiTxEvaluateRequest, WalletAbiTxEvaluateResponse};
pub use schema::filters::{
    WalletAbiAmountFilter, WalletAbiAssetFilter, WalletAbiFinalizerSpec, WalletAbiInputIssuance,
    WalletAbiInputIssuanceKind, WalletAbiInputSchema, WalletAbiInputUnblinding,
    WalletAbiInternalKeySource, WalletAbiLockFilter, WalletAbiTaprootHandle, WalletAbiUtxoSource,
    WalletAbiWalletSourceFilter,
};
pub use schema::outputs::{
    WalletAbiAssetVariant, WalletAbiBlinderVariant, WalletAbiLockVariant, WalletAbiOutputSchema,
    WalletAbiRuntimeParams,
};
pub use schema::preview::{
    WalletAbiPreviewAssetDelta, WalletAbiPreviewOutput, WalletAbiPreviewOutputKind,
    WalletAbiRequestPreview,
};
pub use schema::roots::{
    WalletAbiErrorCode, WalletAbiErrorInfo, WalletAbiStatus, WalletAbiTransactionInfo,
    WalletAbiTxCreateRequest, WalletAbiTxCreateResponse,
};
pub use schema::simf::{
    WalletAbiRuntimeSimfValue, WalletAbiRuntimeSimfWitness, WalletAbiSimfArguments,
    WalletAbiSimfWitness,
};

/// Generate a fresh canonical Wallet ABI request identifier.
#[uniffi::export]
pub fn wallet_abi_generate_request_id() -> String {
    abi::generate_request_id().to_string()
}
