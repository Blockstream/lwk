//! In this module there are wrapper of existing foreign types that:
//! * can be exposed via the bindings
//! * restrict the possible values of the builtin type, for example [`hex::Hex`] restrict the
//! possible values of the builtin String type to only numbers and letter from 'a' to 'e'. Note the
//! restriction is done at usage time, not at instantiation time.

mod asset_id;
mod hex;
mod txid;

pub use asset_id::AssetId;
pub use hex::Hex;
pub use txid::Txid;
