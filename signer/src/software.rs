use bip39::Mnemonic;
use elements_miniscript::elementssig_to_rawsig;
use elements_miniscript::{
    elements::{
        bitcoin::{
            bip32::{self, ExtendedPrivKey, ExtendedPubKey, Fingerprint},
            Network, PrivateKey,
        },
        hashes::Hash,
        pset::PartiallySignedTransaction,
        secp256k1_zkp::{All, Secp256k1},
        sighash::SighashCache,
    },
    psbt::PsbtExt,
};

#[derive(thiserror::Error, Debug)]
pub enum SignError {
    #[error(transparent)]
    Pset(#[from] elements_miniscript::elements::pset::Error),

    #[error(transparent)]
    ElementsEncode(#[from] elements_miniscript::elements::encode::Error),

    #[error(transparent)]
    Sighash(#[from] elements_miniscript::psbt::SighashError),

    #[error(transparent)]
    PsetParse(#[from] elements_miniscript::elements::pset::ParseError),

    #[error(transparent)]
    Bip32(#[from] bip32::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum NewError {
    #[error(transparent)]
    Bip39(#[from] bip39::Error),

    #[error(transparent)]
    Bip32(#[from] bip32::Error),
}

#[derive(Clone)]
pub struct SwSigner<'a> {
    xprv: ExtendedPrivKey,
    secp: &'a Secp256k1<All>, // could be sign only, but it is likely the caller already has the All context.
}

impl<'a> core::fmt::Debug for SwSigner<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signer({})", self.fingerprint())
    }
}

impl<'a> SwSigner<'a> {
    pub fn new(mnemonic: &str, secp: &'a Secp256k1<All>) -> Result<Self, NewError> {
        let mnemonic: Mnemonic = mnemonic.parse()?;
        let xprv = ExtendedPrivKey::new_master(Network::Regtest, &mnemonic.to_seed(""))?;

        Ok(Self { xprv, secp })
    }

    pub fn random(secp: &'a Secp256k1<All>) -> Result<(Self, Mnemonic), NewError> {
        let mnemonic = Mnemonic::generate(12)?;
        let xprv = ExtendedPrivKey::new_master(Network::Regtest, &mnemonic.to_seed(""))?;

        Ok((Self { xprv, secp }, mnemonic))
    }

    pub fn master_xpub(&self) -> ExtendedPubKey {
        ExtendedPubKey::from_priv(self.secp, &self.xprv)
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.xprv.fingerprint(self.secp)
    }
    pub fn sign(&self, pset: &str) -> Result<String, SignError> {
        let mut pset: PartiallySignedTransaction = pset.parse()?;
        self.sign_pset(&mut pset)?;
        Ok(pset.to_string())
    }

    pub fn sign_pset(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, SignError> {
        let tx = pset.extract_tx()?;
        let mut sighash_cache = SighashCache::new(&tx);
        let mut signature_added = 0;

        // genesis hash is not used at all for sighash calculation
        let genesis_hash = elements_miniscript::elements::BlockHash::all_zeros();
        let mut messages = vec![];
        for i in 0..pset.inputs().len() {
            // computing all the messages to sign, it is not necessary if we are not going to sign
            // some input, but since the pset is borrowed, we can't do this action in a inputs_mut() for loop
            let msg = pset
                .sighash_msg(i, &mut sighash_cache, None, genesis_hash)?
                .to_secp_msg();
            messages.push(msg);
        }

        // Fixme: Take a parameter
        let hash_ty = elements_miniscript::elements::EcdsaSighashType::All;

        let signer_fingerprint = self.fingerprint();
        for (input, msg) in pset.inputs_mut().iter_mut().zip(messages) {
            for (want_public_key, (fingerprint, derivation_path)) in input.bip32_derivation.iter() {
                if &signer_fingerprint == fingerprint {
                    let ext_derived = self.xprv.derive_priv(self.secp, derivation_path)?;
                    let private_key = PrivateKey::new(ext_derived.private_key, Network::Bitcoin);
                    let public_key = private_key.public_key(self.secp);
                    if want_public_key == &public_key {
                        // fixme: for taproot use schnorr
                        let sig = self.secp.sign_ecdsa_low_r(&msg, &private_key.inner);
                        let sig = elementssig_to_rawsig(&(sig, hash_ty));

                        let inserted = input.partial_sigs.insert(public_key, sig);
                        if inserted.is_none() {
                            signature_added += 1;
                        }
                    }
                }
            }
        }

        Ok(signature_added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_signer() {
        let secp = Secp256k1::new();
        let signer = SwSigner::new("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about", &secp).unwrap();
        assert_eq!(format!("{:?}", signer), "Signer(73c5da0a)");
        assert_eq!(
            "mnemonic has an invalid word count: 1. Word count must be 12, 15, 18, 21, or 24",
            SwSigner::new("bad", &secp).unwrap_err().to_string()
        );
        assert_eq!("tpubD6NzVbkrYhZ4XYa9MoLt4BiMZ4gkt2faZ4BcmKu2a9te4LDpQmvEz2L2yDERivHxFPnxXXhqDRkUNnQCpZggCyEZLBktV7VaSmwayqMJy1s", &signer.master_xpub().to_string())
    }
}
