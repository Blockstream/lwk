//! Elements block data.
//!
//! This module defines structures contained in the Elements Blockchain
//!

pub mod address;
pub mod address_result;
pub mod block_header;
pub mod external_utxo;
pub mod out_point;
pub mod script;
pub mod transaction;
pub mod tx_in;
#[cfg(feature = "simplicity")]
pub mod tx_in_witness;
pub mod tx_out;
pub mod tx_out_secrets;
pub mod txid;
pub mod wallet_tx;
pub mod wallet_tx_out;
