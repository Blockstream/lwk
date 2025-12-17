//! Contains a software signer [`SwSigner`] and an [`AnySigner`] that can be a Jade or a Software signer.
//!
//! Signers should implement [`lwk_common::Signer`]

#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

mod software;
pub use crate::software::{sign_with_seckey, NewError, SignError, SwSigner};
pub use bip39;

use elements_miniscript::bitcoin::bip32::{self, DerivationPath, Fingerprint};
use elements_miniscript::bitcoin::sign_message::MessageSignature;
use elements_miniscript::elements::bitcoin::bip32::Xpub;
use elements_miniscript::elements::pset::PartiallySignedTransaction;
use lwk_common::Signer;

/// Possible errors when signing with [`AnySigner`]
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum SignerError {
    #[error(transparent)]
    Software(#[from] SignError),

    #[cfg(feature = "jade")]
    #[error(transparent)]
    JadeError(#[from] lwk_jade::error::Error),

    #[cfg(feature = "ledger")]
    #[error(transparent)]
    LedgerError(#[from] lwk_ledger::Error),

    #[error(transparent)]
    Bip32Error(#[from] bip32::Error),
}

/// A signer that can be a software signer [`SwSigner`] or a [`lwk_jade::Jade`]
#[derive(Debug)]
pub enum AnySigner {
    /// A software signer [`SwSigner`]
    Software(SwSigner),

    /// A Jade signer [`lwk_jade::Jade`]
    #[cfg(feature = "jade")]
    Jade(lwk_jade::Jade, elements_miniscript::bitcoin::XKeyIdentifier),

    /// A Ledger signer [`lwk_ledger::Ledger`]
    #[cfg(feature = "ledger")]
    Ledger(
        lwk_ledger::Ledger<lwk_ledger::TransportTcp>,
        elements_miniscript::bitcoin::XKeyIdentifier,
    ),
}

impl Signer for AnySigner {
    type Error = SignerError;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        Signer::sign(&self, pset)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, Self::Error> {
        Signer::derive_xpub(&self, path)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> Result<elements_miniscript::slip77::MasterBlindingKey, Self::Error> {
        Signer::slip77_master_blinding_key(&self)
    }

    fn fingerprint(&self) -> Result<Fingerprint, Self::Error> {
        Signer::fingerprint(&self)
    }

    fn sign_message(
        &self,
        message: &str,
        path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error> {
        Signer::sign_message(&self, message, path)
    }
}

impl Signer for &AnySigner {
    type Error = SignerError;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        Ok(match self {
            AnySigner::Software(signer) => signer.sign(pset)?,

            #[cfg(feature = "jade")]
            AnySigner::Jade(signer, _) => signer.sign(pset)?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(signer, _) => signer.sign(pset)?,
        })
    }

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.derive_xpub(path)?,

            #[cfg(feature = "jade")]
            AnySigner::Jade(s, _) => s.derive_xpub(path)?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(s, _) => s.derive_xpub(path)?,
        })
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> Result<elements_miniscript::slip77::MasterBlindingKey, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.slip77_master_blinding_key()?,

            #[cfg(feature = "jade")]
            AnySigner::Jade(s, _) => s.slip77_master_blinding_key()?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(s, _) => s.slip77_master_blinding_key()?,
        })
    }

    fn fingerprint(&self) -> Result<Fingerprint, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.fingerprint(),

            #[cfg(feature = "jade")]
            AnySigner::Jade(s, _) => s.fingerprint()?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(s, _) => s.fingerprint()?,
        })
    }

    fn sign_message(
        &self,
        message: &str,
        path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.sign_message(message, path)?,

            #[cfg(feature = "jade")]
            AnySigner::Jade(s, _) => s.sign_message(message, path)?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(s, _) => s.sign_message(message, path)?,
        })
    }
}
