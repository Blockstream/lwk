use std::str::FromStr;

use elements::{
    bitcoin::{
        self,
        bip32::{ChildNumber, DerivationPath, Fingerprint, Xpub},
        sign_message::MessageSignature,
        XKeyIdentifier,
    },
    pset::PartiallySignedTransaction,
};
use elements_miniscript::slip77::MasterBlindingKey;

use crate::descriptor::Bip;

/// A trait defining methods of signers, providing blanket implementations for some methods.
pub trait Signer {
    type Error: std::fmt::Debug;

    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error>;

    /// Derive an xpub from the master, path can contains hardened derivations
    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, Self::Error>;

    /// Return the slip77 master blinding key
    fn slip77_master_blinding_key(&self) -> Result<MasterBlindingKey, Self::Error>;

    /// Return the master xpub of the signer
    fn xpub(&self) -> Result<Xpub, Self::Error> {
        self.derive_xpub(&DerivationPath::master())
    }

    /// Return the full identifier of the signer
    fn identifier(&self) -> Result<XKeyIdentifier, Self::Error> {
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
            Bip::Bip49 => format!("49h/{coin_type}h/0h"),
            Bip::Bip87 => format!("87h/{coin_type}h/0h"),
        };

        let fingerprint = self.fingerprint()?;
        let xpub =
            self.derive_xpub(&DerivationPath::from_str(&format!("m/{path}")).expect("static"))?; // TODO avoid string use ChildNumber directly
        let keyorigin_xpub = format!("[{fingerprint}/{path}]{xpub}");
        Ok(keyorigin_xpub)
    }

    fn is_mainnet(&self) -> Result<bool, Self::Error> {
        let xpub = match self.xpub() {
            Ok(xpub) => xpub,
            Err(_) => {
                // We are probably on a Ledger that won't return the master xpub
                let path = [
                    ChildNumber::from_hardened_idx(44).expect("static"),
                    ChildNumber::from_hardened_idx(1).expect("static"), // TODO: work on  mainnet?
                    ChildNumber::from_hardened_idx(0).expect("static"),
                ];
                self.derive_xpub(&DerivationPath::from_iter(path))?
            }
        };
        Ok(xpub.network == bitcoin::NetworkKind::Main)
    }

    fn wpkh_slip77_descriptor(&self) -> Result<String, String> {
        crate::singlesig_desc(
            self,
            crate::Singlesig::Wpkh,
            crate::DescriptorBlindingKey::Slip77,
        )
    }

    /// Sign a message using Bitcoin’s message signing format
    fn sign_message(
        &self,
        message: &str,
        path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error>;
}

#[cfg(feature = "amp0")]
pub mod amp0 {
    use super::*;

    /// AMP0 signer methods
    pub trait Amp0Signer: Signer {
        /// Get AMP0 register xpub
        fn amp0_register_xpub(&self) -> Result<Xpub, Self::Error> {
            let path = DerivationPath::from_str("m/18241h").expect("static");
            self.derive_xpub(&path)
        }
    }
}
