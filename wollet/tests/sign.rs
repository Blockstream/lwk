use crate::bitcoin::bip32::{ExtendedPubKey, Fingerprint};
use jade::lock_jade::LockJade;
use software_signer::SwSigner;
use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Software(#[from] software_signer::SignError),

    #[error(transparent)]
    Jade(#[from] jade::sign_pset::Error),

    #[error(transparent)]
    JadeError(#[from] jade::error::Error),

    #[error(transparent)]
    Bip32Error(#[from] crate::bitcoin::bip32::Error),
}

pub trait Sign {
    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut elements::pset::PartiallySignedTransaction) -> Result<u32, Error>;
}

pub enum Signer<'a> {
    Software(SwSigner<'a>),
    Jade(&'a LockJade),
}

impl<'a> Signer<'a> {
    pub fn xpub(&self) -> Result<ExtendedPubKey, Error> {
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
}

impl Sign for LockJade {
    fn sign(&self, pset: &mut elements::pset::PartiallySignedTransaction) -> Result<u32, Error> {
        Ok(self.sign_pset(pset)?)
    }
}

impl<'a> Sign for SwSigner<'a> {
    fn sign(&self, pset: &mut elements::pset::PartiallySignedTransaction) -> Result<u32, Error> {
        Ok(self.sign_pset(pset)?)
    }
}

impl<'a> Sign for Signer<'a> {
    fn sign(&self, pset: &mut elements::pset::PartiallySignedTransaction) -> Result<u32, Error> {
        Sign::sign(&self, pset)
    }
}

impl<'a> Sign for &Signer<'a> {
    fn sign(&self, pset: &mut elements::pset::PartiallySignedTransaction) -> Result<u32, Error> {
        Ok(match self {
            Signer::Software(signer) => signer.sign_pset(pset)?,
            Signer::Jade(signer) => signer.sign_pset(pset)?,
        })
    }
}
