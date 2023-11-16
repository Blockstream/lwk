use elements::{
    bitcoin::bip32::{DerivationPath, ExtendedPubKey},
    pset::PartiallySignedTransaction,
};

pub trait Signer {
    type Error: std::fmt::Debug;

    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error>;

    /// Derive an xpub from the master, path can contains hardened derivations
    fn derive_xpub(&self, path: &DerivationPath) -> Result<ExtendedPubKey, Self::Error>;
}
