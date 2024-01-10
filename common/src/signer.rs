use std::str::FromStr;

use elements::{
    bitcoin::{
        bip32::{DerivationPath, ExtendedPubKey, Fingerprint},
        hash_types::XpubIdentifier,
    },
    pset::PartiallySignedTransaction,
};
use elements_miniscript::slip77::MasterBlindingKey;

use crate::descriptor::Bip;

pub trait Signer {
    type Error: std::fmt::Debug;

    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error>;

    /// Derive an xpub from the master, path can contains hardened derivations
    fn derive_xpub(&self, path: &DerivationPath) -> Result<ExtendedPubKey, Self::Error>;

    /// Return the slip77 master blinding key
    fn slip77_master_blinding_key(&self) -> Result<MasterBlindingKey, Self::Error>;

    /// Return the master xpub of the signer
    fn xpub(&self) -> Result<ExtendedPubKey, Self::Error> {
        self.derive_xpub(&DerivationPath::master())
    }

    /// Return the full identifier of the signer
    fn identifier(&self) -> Result<XpubIdentifier, Self::Error> {
        Ok(self.xpub()?.identifier())
    }

    /// Return the fingerprint of the signer (4 bytes)
    fn fingerprint(&self) -> Result<Fingerprint, Self::Error> {
        Ok(self.xpub()?.fingerprint())
    }

    /// Return keyorigin and xpub, like "[73c5da0a/84h/1h/0h]tpub..."
    fn keyorigin_xpub(&self, bip: Bip, is_mainnet: bool) -> Result<String, Self::Error> {
        let coin_type = if is_mainnet { 1776 } else { 1 };
        let path = match bip {
            Bip::Bip84 => format!("84h/{coin_type}h/0h"),
        };

        let fingerprint = self.fingerprint()?;
        let xpub =
            self.derive_xpub(&DerivationPath::from_str(&format!("m/{path}")).expect("static"))?; // TODO avoid string use ChildNumber directly
        let keyorigin_xpub = format!("[{fingerprint}/{path}]{xpub}");
        Ok(keyorigin_xpub)
    }
}
