pub use crate::psbt::PartialSignature;
pub use client::{LiquidClient, Transport};
use elements_miniscript::bitcoin::bip32::DerivationPath;
pub use transport_tcp::TransportTcp;

mod client;
mod transport_tcp;

#[derive(Debug)]
pub struct Ledger<T: Transport> {
    /// Ledger Liquid Client
    pub client: LiquidClient<T>,
}

impl Ledger<TransportTcp> {
    pub fn new(port: u16) -> Self {
        let client = LiquidClient::new(TransportTcp::new(port).expect("TODO"));
        Self { client }
    }
}
impl<T: Transport> Ledger<T> {
    pub fn from_transport(transport: T) -> Self {
        let client = LiquidClient::new(transport);
        Self { client }
    }
}

pub enum Singlesig {
    Wpkh,
}

impl Singlesig {
    pub fn derivation_path(&self) -> DerivationPath {
        // TODO network
        match self {
            Singlesig::Wpkh => "m/84h/1h/0h".parse().expect("static"),
        }
    }
}
