//! Runtime transaction resolution stack for `wallet-abi-0.1`.
//!
//! Key stages:
//! - input resolution and deficit funding
//! - output balancing and change/fee materialization
//! - finalization and Simplicity witness resolution
//!
//! Compatibility notes:
//! - fee rate values are interpreted as sat/kvB in runtime internals
//! - blinded outputs currently require at least one wallet-finalized input as
//!   `blinder_index` anchor

mod bnb;
mod input_resolution;
mod output_resolution;
mod utils;

pub mod runtime;

use utils::{get_finalizer_spec_key, get_secrets_spec_key};
