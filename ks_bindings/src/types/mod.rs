//! In this module there are wrapper of existing foreign types that:
//! * can be exposed via the bindings
//! * restrict the possible values of the builtin type, for example [`hex::Hex`] restrict the
//! possible values of the builtin String type to only numbers and letter from 'a' to 'e'

pub mod hex;
pub mod txid;
