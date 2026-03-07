//! Wallet ABI surface for `wallet-abi-0.1`.
//!
//! This module exposes:
//! - schema payload types/codecs (`schema`)
//! - runtime transaction resolution/finalization (`tx_resolution`)
//!
//! Runtime note:
//! for `wallet-abi-0.1`, blinded outputs require at least one wallet-finalized
//! input as `blinder_index` anchor.

pub mod schema;
pub mod tx_resolution;

#[cfg(feature = "wallet_abi_test_utils")]
pub mod test_utils;
