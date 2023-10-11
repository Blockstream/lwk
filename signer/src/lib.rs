mod software;

pub use crate::software::{NewError, SignError, SwSigner};

use elements_miniscript::elements;
use elements_miniscript::elements::bitcoin::bip32::{ExtendedPubKey, Fingerprint};
use elements_miniscript::elements::pset::PartiallySignedTransaction;
use jade::lock_jade::LockJade;
use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
pub enum SignerError {
    #[error(transparent)]
    Software(#[from] SignError),

    #[error(transparent)]
    Jade(#[from] jade::sign_pset::Error),

    #[error(transparent)]
    JadeError(#[from] jade::error::Error),

    #[error(transparent)]
    Bip32Error(#[from] elements::bitcoin::bip32::Error),
}

pub trait Sign {
    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, SignerError>;
}

pub enum Signer<'a> {
    Software(SwSigner<'a>),
    Jade(&'a LockJade),
}

impl<'a> Signer<'a> {
    pub fn xpub(&self) -> Result<ExtendedPubKey, SignerError> {
        match self {
            Signer::Software(s) => Ok(s.xpub()),
            Signer::Jade(s) => {
                let params = jade::protocol::GetXpubParams {
                    network: jade::Network::LocaltestLiquid,
                    path: vec![],
                };
                let result = s.get_xpub(params)?;
                Ok(ExtendedPubKey::from_str(result.get())?)
            }
        }
    }

    pub fn fingerprint(&self) -> Result<Fingerprint, SignerError> {
        Ok(self.xpub()?.fingerprint())
    }
}

impl Sign for LockJade {
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, SignerError> {
        Ok(self.sign_pset(pset)?)
    }
}

impl<'a> Sign for SwSigner<'a> {
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, SignerError> {
        Ok(self.sign_pset(pset)?)
    }
}

impl<'a> Sign for Signer<'a> {
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, SignerError> {
        Sign::sign(&self, pset)
    }
}

impl<'a> Sign for &Signer<'a> {
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, SignerError> {
        Ok(match self {
            Signer::Software(signer) => signer.sign_pset(pset)?,
            Signer::Jade(signer) => signer.sign_pset(pset)?,
        })
    }
}
