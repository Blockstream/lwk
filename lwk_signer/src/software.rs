use bip39::Mnemonic;
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
use lwk_common::Signer;

/// Possible errors when signing with the software signer [`SwSigner`]
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

    #[error("Cannot derive slip77 key (mnemonic/seed not available)")]
    DeterministicSlip77NotAvailable,
}

/// Possible errors when creating a new software signer [`SwSigner`]
#[derive(thiserror::Error, Debug)]
pub enum NewError {
    #[error(transparent)]
    Bip39(#[from] bip39::Error),

    #[error(transparent)]
    Bip32(#[from] bip32::Error),
}

/// Options for ECDSA signing
#[derive(Clone, Debug, Default)]
enum EcdsaSignOpt {
    /// Create signature with low r
    #[default]
    LowR,

    /// Create signatures without grinding the nonce
    NoGrind,
}

/// A software signer
#[derive(Clone)]
pub struct SwSigner {
    pub(crate) xprv: Xpriv,
    pub(crate) secp: Secp256k1<All>, // could be sign only, but it is likely the caller already has the All context.
    pub(crate) mnemonic: Option<Mnemonic>,
    ecdsa_sign_opt: EcdsaSignOpt,
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
            mnemonic: Some(mnemonic),
            ecdsa_sign_opt: EcdsaSignOpt::default(),
        })
    }

    pub fn is_mainnet(&self) -> bool {
        self.xprv.network == bitcoin::NetworkKind::Main
    }

    pub fn random(is_mainnet: bool) -> Result<(Self, Mnemonic), NewError> {
        let mnemonic = Mnemonic::generate(12)?;
        Ok((SwSigner::new(&mnemonic.to_string(), is_mainnet)?, mnemonic))
    }

    pub fn from_xprv(xprv: Xpriv) -> Self {
        Self {
            xprv,
            secp: Secp256k1::new(),
            mnemonic: None,
            ecdsa_sign_opt: EcdsaSignOpt::default(),
        }
    }

    /// Produce "low R" ECDSA signatures (default and recommended option)
    pub fn set_ecdsa_sign_low_r(&mut self) {
        self.ecdsa_sign_opt = EcdsaSignOpt::LowR;
    }

    /// Produce no grind "R" in ECDSA signatures
    pub fn set_ecdsa_sign_no_grind(&mut self) {
        self.ecdsa_sign_opt = EcdsaSignOpt::NoGrind;
    }

    pub fn xpub(&self) -> Xpub {
        Xpub::from_priv(&self.secp, &self.xprv)
    }

    pub fn seed(&self) -> Option<[u8; 64]> {
        self.mnemonic.as_ref().map(|m| m.to_seed(""))
    }

    pub fn mnemonic(&self) -> Option<Mnemonic> {
        self.mnemonic.clone()
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.xprv.fingerprint(&self.secp)
    }

    pub fn derive_xprv(&self, path: &DerivationPath) -> Result<Xpriv, SignError> {
        Ok(self.xprv.derive_priv(&self.secp, path)?)
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
                        let sig = match self.ecdsa_sign_opt {
                            EcdsaSignOpt::LowR => {
                                self.secp.sign_ecdsa_low_r(&msg, &private_key.inner)
                            }
                            EcdsaSignOpt::NoGrind => self.secp.sign_ecdsa(&msg, &private_key.inner),
                        };
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
        let seed = self
            .seed()
            .ok_or_else(|| SignError::DeterministicSlip77NotAvailable)?;
        Ok(MasterBlindingKey::from_seed(&seed[..]))
    }
}

#[cfg(test)]
mod tests {
    use elements_miniscript::elements::hex::ToHex;

    use super::*;

    #[test]
    fn new_signer() {
        let signer = SwSigner::new(lwk_test_util::TEST_MNEMONIC, false).unwrap();
        assert_eq!(format!("{:?}", signer), "Signer(73c5da0a)");
        assert_eq!(
            "mnemonic has an invalid word count: 1. Word count must be 12, 15, 18, 21, or 24",
            SwSigner::new("bad", false).expect_err("test").to_string()
        );
        assert_eq!(
            lwk_test_util::TEST_MNEMONIC_XPUB,
            &signer.xpub().to_string()
        );

        let slip77 = signer.slip77_master_blinding_key().unwrap();
        assert_eq!(slip77.as_bytes().to_hex(), format!("{}", slip77));
        assert_eq!(
            slip77.as_bytes().to_hex(),
            lwk_test_util::TEST_MNEMONIC_SLIP77
        );

        let path: DerivationPath = "m/0'".parse().unwrap();
        let xprv = signer.derive_xprv(&path).unwrap();
        let xpub = signer.derive_xpub(&path).unwrap();
        let secp = Secp256k1::new();
        assert_eq!(xpub, Xpub::from_priv(&secp, &xprv));
    }

    #[test]
    fn from_xprv() {
        use std::str::FromStr;
        let xprv = Xpriv::from_str("tprv8bxtvyWEZW9M4n8ByZVSG2NNP4aeiRdhDZXNEv1eVNtrhLLnc6vJ1nf9DN5cHAoxMwqRR1CD6YXBvw2GncSojF8DknPnQVMgbpkjnKHkrGY").unwrap();
        let xpub = Xpub::from_str("tpubD8ew5PYUhsq1xF9ysDA2fS2Ux66askpbns89XS3wuehFXpbZEVjtCHH1PUhj6KAfCs4iCx5wKgswv1n3we2ZHEs2sP5pw9PnLsCFwiVgdjw").unwrap();
        let signer = SwSigner::from_xprv(xprv);
        assert_eq!(signer.xpub(), xpub);
        assert!(signer.mnemonic().is_none());
        assert!(signer.seed().is_none());
    }

    #[test]
    fn signer_ecdsa_opt() {
        // Sign with the default option (low R) and then with the "no grind" option
        let mut signer = SwSigner::new(lwk_test_util::TEST_MNEMONIC, false).unwrap();
        let b64 = include_str!("../../lwk_jade/test_data/pset_to_be_signed.base64");
        let mut pset_low_r: PartiallySignedTransaction = b64.parse().unwrap();
        let sig_added = signer.sign(&mut pset_low_r).unwrap();
        assert_eq!(sig_added, 1);

        signer.set_ecdsa_sign_no_grind();
        let mut pset_no_grind: PartiallySignedTransaction = b64.parse().unwrap();
        let sig_added = signer.sign(&mut pset_no_grind).unwrap();
        assert_eq!(sig_added, 1);

        // In the case the signatures are different, but in general signatures might not
        // differ, since the grinding for low R might not be necessary.
        assert_ne!(pset_no_grind, pset_low_r);
        let sig_no_grind = pset_no_grind.inputs()[0]
            .partial_sigs
            .values()
            .next()
            .unwrap();
        let sig_low_r = pset_low_r.inputs()[0].partial_sigs.values().next().unwrap();
        assert_ne!(sig_low_r, sig_no_grind);
        assert!(sig_low_r.len() < sig_no_grind.len());
    }
}
