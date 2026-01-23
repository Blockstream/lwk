//! In this module there are wrapper of existing foreign simple types that:
//! * can be exposed via the bindings
//! * restrict the possible values of the builtin type, for example [`hex::Hex`] restrict the
//!   possible values of the builtin String type to only numbers and letter from 'a' to 'e'. Note the
//!   restriction is done at usage time, not at instantiation time.

mod asset_id;
mod hex;
mod secret_key;
mod xonly_public_key;

pub use asset_id::AssetId;
pub use hex::Hex;
pub use secret_key::SecretKey;
pub use xonly_public_key::XOnlyPublicKey;
