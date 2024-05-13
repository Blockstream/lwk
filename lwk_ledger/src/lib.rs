mod apdu;
mod client;
mod command;
mod error;
mod interpreter;
mod merkle;
mod psbt;
mod transport;
mod wallet;

// Adapted from
// https://github.com/LedgerHQ/app-bitcoin-new/tree/master/bitcoin_client_rs
pub use wallet::{AddressType, Version, WalletPolicy, WalletPubKey};
pub use client::LiquidClient;
pub use transport::TransportTcp;

pub fn new(port: u16) -> LiquidClient<TransportTcp> {
    LiquidClient::new(TransportTcp::new(port).expect("TODO"))
}

#[derive(Debug)]
pub struct Ledger {
    /// Ledger Liquid Client
    pub client: LiquidClient<TransportTcp>,
}
