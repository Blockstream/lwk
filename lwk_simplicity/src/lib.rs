//! WARNING: NOT PRODUCTION READY.
//!
//! This crate and all functionality behind the `simplicity` feature flag is
//! intended only for tinkering and experimentation with Simplicity. APIs may
//! change or be removed without notice. Do not use in production environments
//! or with real funds.

pub mod error;
#[cfg(feature = "lending")]
pub mod lending;
pub mod runner;
pub mod scripts;
pub mod signer;
pub mod taproot_pubkey_gen;
pub mod wallet_abi;

// Re-export simplicityhl crate for bindings
pub use simplicityhl;
