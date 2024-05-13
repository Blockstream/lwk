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
pub use client::LiquidClient;
pub use transport::TransportTcp;
pub use wallet::{AddressType, Version, WalletPolicy, WalletPubKey};

use elements_miniscript::confidential::slip77;
use elements_miniscript::elements::bitcoin::bip32::{DerivationPath, Xpub};
use elements_miniscript::elements::pset::PartiallySignedTransaction;

use lwk_common::Signer;

#[derive(Debug)]
pub struct Ledger {
    /// Ledger Liquid Client
    pub client: LiquidClient<TransportTcp>,
}

impl Ledger {
    pub fn new(port: u16) -> Self {
        let client = LiquidClient::new(TransportTcp::new(port).expect("TODO"));
        Self { client }
    }
}

pub type Error = error::LiquidClientError<TransportTcp>;

impl Signer for &Ledger {
    type Error = crate::Error;

    fn sign(
        &self,
        _pset: &mut PartiallySignedTransaction,
    ) -> std::result::Result<u32, Self::Error> {
        todo!();
    }

    fn derive_xpub(&self, path: &DerivationPath) -> std::result::Result<Xpub, Self::Error> {
        let r = self.client.get_extended_pubkey(path, false).expect("FIXME");
        Ok(r)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> std::result::Result<slip77::MasterBlindingKey, Self::Error> {
        todo!();
    }
}

impl Signer for Ledger {
    type Error = crate::Error;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> std::result::Result<u32, Self::Error> {
        Signer::sign(&self, pset)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> std::result::Result<Xpub, Self::Error> {
        Signer::derive_xpub(&self, path)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> std::result::Result<slip77::MasterBlindingKey, Self::Error> {
        Signer::slip77_master_blinding_key(&self)
    }
}
