mod apdu;
mod client;
mod command;
mod error;
mod interpreter;
mod merkle;
mod transport;
mod wallet;

// Adapted from
// https://github.com/LedgerHQ/app-bitcoin-new/tree/master/bitcoin_client_rs
use crate::client::LiquidClient;
use transport::TransportTcp;

pub use wallet::{Version, WalletPolicy, WalletPubKey};

pub fn new(port: u16) -> LiquidClient<TransportTcp> {
    LiquidClient::new(TransportTcp::new(port).expect("TODO"))
}
