//! In this module there are wrapper of existing foreign simple types that:
//! * can be exposed via the bindings
//! * restrict the possible values of the builtin type, for example [`hex::Hex`] restrict the
//!   possible values of the builtin String type to only numbers and letter from 'a' to 'e'. Note the
//!   restriction is done at usage time, not at instantiation time.

mod asset_id;
mod blinding_factor;
#[cfg(feature = "simplicity")]
mod contract_hash;
#[cfg(feature = "simplicity")]
mod control_block;
mod hex;
#[cfg(feature = "simplicity")]
mod keypair;
#[cfg(feature = "simplicity")]
mod lock_time;
#[cfg(feature = "simplicity")]
mod public_key;
mod secret_key;
#[cfg(feature = "simplicity")]
mod tweak;
#[cfg(feature = "simplicity")]
mod tx_sequence;
#[cfg(feature = "simplicity")]
mod xonly_public_key;

pub use asset_id::AssetId;
#[cfg(feature = "simplicity")]
pub use asset_id::{asset_id_from_issuance, asset_id_inner_hex, reissuance_token_from_issuance};
#[cfg(feature = "simplicity")]
pub use blinding_factor::{AssetBlindingFactor, ValueBlindingFactor};
#[cfg(feature = "simplicity")]
pub use contract_hash::ContractHash;
#[cfg(feature = "simplicity")]
pub use control_block::ControlBlock;
pub use hex::Hex;
#[cfg(feature = "simplicity")]
pub use keypair::Keypair;
#[cfg(feature = "simplicity")]
pub use lock_time::LockTime;
#[cfg(feature = "simplicity")]
pub use public_key::PublicKey;
pub use secret_key::SecretKey;
#[cfg(feature = "simplicity")]
pub use tweak::Tweak;
#[cfg(feature = "simplicity")]
pub use tx_sequence::TxSequence;
#[cfg(feature = "simplicity")]
pub use xonly_public_key::XOnlyPublicKey;
