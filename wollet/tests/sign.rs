use jade::lock_jade::LockJade;
use software_signer::SwSigner;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Software(#[from] software_signer::SignError),

    #[error(transparent)]
    Jade(#[from] jade::sign_pset::Error),
}

pub trait Sign {
    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut elements::pset::PartiallySignedTransaction) -> Result<u32, Error>;
}

pub enum Signer<'a> {
    Software(SwSigner<'a>),
    Jade(LockJade),
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
        Ok(match self {
            Signer::Software(signer) => signer.sign_pset(pset)?,
            Signer::Jade(signer) => signer.sign_pset(pset)?,
        })
    }
}
