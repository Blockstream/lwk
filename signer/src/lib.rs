#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Contains a software signer [`SwSigner`] and an [`AnySigner`] that can be a Jade or a Software signer

mod software;

pub use crate::software::{NewError, SignError, SwSigner};
pub use bip39;

use common::Signer;
use elements_miniscript::bitcoin::bip32::DerivationPath;
use elements_miniscript::bitcoin::XKeyIdentifier;
use elements_miniscript::elements;
use elements_miniscript::elements::bitcoin::bip32::Xpub;
use elements_miniscript::elements::pset::PartiallySignedTransaction;
use jade::mutex_jade::MutexJade;

#[derive(thiserror::Error, Debug)]
pub enum SignerError {
    #[error(transparent)]
    Software(#[from] SignError),

    #[error(transparent)]
    JadeError(#[from] jade::error::Error),

    #[error(transparent)]
    Bip32Error(#[from] elements::bitcoin::bip32::Error),
}

#[derive(Debug)]
pub enum AnySigner {
    Software(SwSigner),
    Jade(MutexJade, XKeyIdentifier),
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
}

impl Signer for &AnySigner {
    type Error = SignerError;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        Ok(match self {
            AnySigner::Software(signer) => signer.sign(pset)?,
            AnySigner::Jade(signer, _) => signer.sign(pset)?,
        })
    }

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.derive_xpub(path)?,
            AnySigner::Jade(s, _) => s.derive_xpub(path)?,
        })
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> Result<elements_miniscript::slip77::MasterBlindingKey, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.slip77_master_blinding_key()?,
            AnySigner::Jade(s, _) => s.slip77_master_blinding_key()?,
        })
    }
}
