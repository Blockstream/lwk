pub use crate::psbt::PartialSignature;
pub use client::{LiquidClient, Transport};
use elements_miniscript::{
    bitcoin::bip32::{DerivationPath, Fingerprint, Xpub},
    slip77,
};
pub use transport_tcp::TransportTcp;

use crate::Error;

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

/// TODO Should implement Signer, but here we are async. Make async signer and impl here and for jade
impl Ledger<TransportTcp> {
    pub async fn derive_xpub(&self, path: &DerivationPath) -> std::result::Result<Xpub, Error> {
        let r = self
            .client
            .get_extended_pubkey(path, false)
            .await
            .expect("FIXME");
        Ok(r)
    }

    pub async fn slip77_master_blinding_key(
        &self,
    ) -> std::result::Result<slip77::MasterBlindingKey, Error> {
        let r = self.client.get_master_blinding_key().await.expect("FIXME");
        Ok(r)
    }

    pub async fn fingerprint(&self) -> std::result::Result<Fingerprint, Error> {
        let r = self.client.get_master_fingerprint().await.expect("FIXME");
        Ok(r)
    }
}
