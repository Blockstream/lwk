use elements::{
    bitcoin::{
        bip32::{DerivationPath, ExtendedPubKey, Fingerprint},
        hash_types::XpubIdentifier,
    },
    pset::PartiallySignedTransaction,
};
use elements_miniscript::slip77::MasterBlindingKey;

pub trait Signer {
    type Error: std::fmt::Debug;

    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error>;

    /// Derive an xpub from the master, path can contains hardened derivations
    fn derive_xpub(&self, path: &DerivationPath) -> Result<ExtendedPubKey, Self::Error>;

    /// Return the slip77 master blinding key
    fn slip77_master_blinding_key(&self) -> Result<MasterBlindingKey, Self::Error>;

    fn xpub(&self) -> Result<ExtendedPubKey, Self::Error> {
        self.derive_xpub(&DerivationPath::master())
    }

    fn identifier(&self) -> Result<XpubIdentifier, Self::Error> {
        Ok(self.xpub()?.identifier())
    }

    fn fingerprint(&self) -> Result<Fingerprint, Self::Error> {
        Ok(self.derive_xpub(&DerivationPath::master())?.fingerprint())
    }
}
