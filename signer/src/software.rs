use bip39::Mnemonic;
use common::Signer;
use elements_miniscript::{
    bitcoin::{self, bip32::DerivationPath, PrivateKey},
    elements::{
        bitcoin::{
            bip32::{self, Fingerprint, Xpriv, Xpub},
            Network,
        },
        hashes::Hash,
        pset::PartiallySignedTransaction,
        secp256k1_zkp::{All, Secp256k1},
        sighash::SighashCache,
    },
    elementssig_to_rawsig,
    psbt::PsbtExt,
    slip77::MasterBlindingKey,
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
pub struct SwSigner {
    pub(crate) xprv: Xpriv,
    pub(crate) secp: Secp256k1<All>, // could be sign only, but it is likely the caller already has the All context.
    pub(crate) mnemonic: Mnemonic,
}

impl core::fmt::Debug for SwSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signer({})", self.fingerprint())
    }
}

impl SwSigner {
    /// Creates a new software signer from the given mnemonic.
    ///
    /// Takes also a flag if the network is mainnet so that generated extended keys are in the
    /// correct form xpub/tpub (there is no need to discriminate between regtest and testnet)
    pub fn new(mnemonic: &str, is_mainnet: bool) -> Result<Self, NewError> {
        let secp = Secp256k1::new();
        let mnemonic: Mnemonic = mnemonic.parse()?;
        let seed = mnemonic.to_seed("");

        let network = if is_mainnet {
            bitcoin::Network::Bitcoin
        } else {
            bitcoin::Network::Testnet
        };

        let xprv = Xpriv::new_master(network, &seed)?;

        Ok(Self {
            xprv,
            secp,
            mnemonic,
        })
    }

    pub fn random(is_mainnet: bool) -> Result<(Self, Mnemonic), NewError> {
        let mnemonic = Mnemonic::generate(12)?;
        Ok((SwSigner::new(&mnemonic.to_string(), is_mainnet)?, mnemonic))
    }

    pub fn xpub(&self) -> Xpub {
        Xpub::from_priv(&self.secp, &self.xprv)
    }

    pub fn seed(&self) -> [u8; 64] {
        self.mnemonic.to_seed("")
    }

    pub fn mnemonic(&self) -> Mnemonic {
        self.mnemonic.clone()
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.xprv.fingerprint(&self.secp)
    }
}

impl Signer for SwSigner {
    type Error = SignError;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
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
                    let ext_derived = self.xprv.derive_priv(&self.secp, derivation_path)?;
                    let private_key = PrivateKey::new(ext_derived.private_key, Network::Bitcoin);
                    let public_key = private_key.public_key(&self.secp);
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

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, Self::Error> {
        let derived = self.xprv.derive_priv(&self.secp, path)?;
        Ok(Xpub::from_priv(&self.secp, &derived))
    }

    fn slip77_master_blinding_key(&self) -> Result<MasterBlindingKey, Self::Error> {
        Ok(MasterBlindingKey::from_seed(&self.seed()[..]))
    }
}

#[cfg(test)]
mod tests {
    use elements_miniscript::elements::hex::ToHex;

    use super::*;

    #[test]
    fn new_signer() {
        let signer = SwSigner::new(test_util::TEST_MNEMONIC, false).unwrap();
        assert_eq!(format!("{:?}", signer), "Signer(73c5da0a)");
        assert_eq!(
            "mnemonic has an invalid word count: 1. Word count must be 12, 15, 18, 21, or 24",
            SwSigner::new("bad", false).expect_err("test").to_string()
        );
        assert_eq!(test_util::TEST_MNEMONIC_XPUB, &signer.xpub().to_string());

        let slip77 = signer.slip77_master_blinding_key().unwrap();
        assert_eq!(slip77.as_bytes().to_hex(), test_util::TEST_MNEMONIC_SLIP77);
    }
}
