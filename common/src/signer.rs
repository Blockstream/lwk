use elements::pset::PartiallySignedTransaction;

pub trait Signer {
    type Error: std::fmt::Debug;

    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error>;
}
