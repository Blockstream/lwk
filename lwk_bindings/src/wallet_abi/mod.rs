use std::str::FromStr;
use std::sync::Arc;

use crate::simplicity::{SimplicityArguments, SimplicityWitnessValues};
use crate::types::{AssetId, LockTime, PublicKey, SecretKey, TxSequence, XOnlyPublicKey};
use crate::{LwkError, Network, OutPoint, Script, Txid};

use lwk_simplicity::taproot_pubkey_gen::TaprootPubkeyGen;
use lwk_simplicity::wallet_abi::schema as abi;

mod conversions;
mod schema;

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
