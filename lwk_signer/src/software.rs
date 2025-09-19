use bip39::Mnemonic;
use elements_miniscript::{
    bitcoin::{
        self,
        bip32::DerivationPath,
        secp256k1::Message,
        sign_message::{MessageSignature, MessageSignatureError},
        PrivateKey,
    },
    elements::{
        bitcoin::{
            bip32::{self, Fingerprint, Xpriv, Xpub},
            Network,
        },
        hashes::Hash,
        pset::PartiallySignedTransaction,
        secp256k1_zkp::{All, Secp256k1},
        sighash::SighashCache,
        EcdsaSighashType,
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
    /// Error parsing the mnemonic
    #[error(transparent)]
    Bip39(#[from] bip39::Error),

    /// Error deriving the extended private key
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

    /// Return true if the signer is for mainnet. There is no need to discriminate between regtest and testnet.
    pub fn is_mainnet(&self) -> bool {
        self.xprv.network == bitcoin::NetworkKind::Main
    }

    /// Create a new software signer from a random mnemonic
    pub fn random(is_mainnet: bool) -> Result<(Self, Mnemonic), NewError> {
        let mnemonic = Mnemonic::generate(12)?;
        Ok((SwSigner::new(&mnemonic.to_string(), is_mainnet)?, mnemonic))
    }

    /// Create a new software signer from a given extended private key
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

    /// Return the extended public key of the signer
    pub fn xpub(&self) -> Xpub {
        Xpub::from_priv(&self.secp, &self.xprv)
    }

    /// Return the seed of the signer if it has been initialized with the mnemonic (can't provide the seed if initialized with xprv)
    pub fn seed(&self) -> Option<[u8; 64]> {
        self.mnemonic.as_ref().map(|m| m.to_seed(""))
    }

    /// Return the mnemonic of the signer if it has been initialized with the mnemonic (can't provide the mnemonic if initialized with xprv)
    pub fn mnemonic(&self) -> Option<Mnemonic> {
        self.mnemonic.clone()
    }

    /// Return the fingerprint of the signer (4 bytes)
    pub fn fingerprint(&self) -> Fingerprint {
        self.xprv.fingerprint(&self.secp)
    }

    /// Derive an xprv from the master, path can contains hardened derivations.
    pub fn derive_xprv(&self, path: &DerivationPath) -> Result<Xpriv, SignError> {
        Ok(self.xprv.derive_priv(&self.secp, path)?)
    }
}

#[allow(dead_code)]
fn verify(
    secp: &Secp256k1<All>,
    address: &bitcoin::Address,
    message: &str,
    signature: &MessageSignature,
) -> Result<bool, MessageSignatureError> {
    let msg_hash = bitcoin::sign_message::signed_msg_hash(message);
    signature.is_signed_by_address(secp, address, msg_hash)
}

#[allow(dead_code)]
fn p2pkh(xpub: &Xpub) -> bitcoin::Address {
    let bitcoin_pubkey = bitcoin::PublicKey::new(xpub.public_key);
    bitcoin::Address::p2pkh(bitcoin_pubkey, xpub.network)
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

        let signer_fingerprint = self.fingerprint();
        for (input, msg) in pset.inputs_mut().iter_mut().zip(messages) {
            let hash_ty = input
                .sighash_type
                .map(|h| h.ecdsa_hash_ty().unwrap_or(EcdsaSighashType::All))
                .unwrap_or(EcdsaSighashType::All);
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

    fn sign_message(
        &self,
        message: &str,
        path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error> {
        let digest = bitcoin::sign_message::signed_msg_hash(message);
        let message = Message::from_digest_slice(digest.as_ref()).expect("digest is 32");
        let derived = self.xprv.derive_priv(&self.secp, path)?;
        let signature = self
            .secp
            .sign_ecdsa_recoverable(&message, &derived.private_key);
        let signature = MessageSignature {
            signature,
            compressed: true,
        };
        Ok(signature)
    }
}

#[cfg(feature = "amp0")]
impl lwk_common::Amp0Signer for SwSigner {}

/// Sign a PSET with a given secret key
pub fn sign_with_seckey(
    seckey: bitcoin::secp256k1::SecretKey,
    pset: &mut PartiallySignedTransaction,
) -> Result<u32, SignError> {
    // TODO: share code with fn.sign above
    let secp = Secp256k1::new();
    let signing_pk = seckey.public_key(&secp);
    let signing_pk = bitcoin::key::PublicKey::new(signing_pk);

    let tx = pset.extract_tx()?;
    let mut sighash_cache = SighashCache::new(&tx);
    let mut signature_added = 0;
    let genesis_hash = elements_miniscript::elements::BlockHash::all_zeros();
    let mut messages = vec![];
    for i in 0..pset.inputs().len() {
        let msg = pset
            .sighash_msg(i, &mut sighash_cache, None, genesis_hash)?
            .to_secp_msg();
        messages.push(msg);
    }

    for (input, msg) in pset.inputs_mut().iter_mut().zip(messages) {
        let hash_ty = input
            .sighash_type
            .map(|h| h.ecdsa_hash_ty().unwrap_or(EcdsaSighashType::All))
            .unwrap_or(EcdsaSighashType::All);
        for pk in input.bip32_derivation.keys() {
            if pk == &signing_pk {
                let sig = secp.sign_ecdsa_low_r(&msg, &seckey);
                let sig = elementssig_to_rawsig(&(sig, hash_ty));

                let inserted = input.partial_sigs.insert(signing_pk, sig);
                if inserted.is_none() {
                    signature_added += 1;
                }
            }
        }
    }

    Ok(signature_added)
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

    #[test]
    fn test_sign_verify() {
        let signer = SwSigner::new(lwk_test_util::TEST_MNEMONIC, true).unwrap();
        let message = "Hello, world!";
        let path = DerivationPath::master();
        let signature = signer.sign_message(message, &path).unwrap();
        let xpub = signer.derive_xpub(&path).unwrap();
        let address = p2pkh(&xpub);
        assert_eq!(address.to_string(), "1BZ9j3F7m4H1RPyeDp5iFwpR31SB6zrs19");
        assert_eq!(signature.to_string(), "Hwlg40qLYZXEj9AoA3oZpfJMJPxaXzBL0+siHAJRhTIvSFiwSdtCsqxqB7TxgWfhqIr/YnGE4nagWzPchFJElTo=");
        let verified = verify(&signer.secp, &address, message, &signature).unwrap();
        assert!(verified);

        // result checked also with bitcoin-cli
        // bitcoin-cli verifymessage "1BZ9j3F7m4H1RPyeDp5iFwpR31SB6zrs19" "Hwlg40qLYZXEj9AoA3oZpfJMJPxaXzBL0+siHAJRhTIvSFiwSdtCsqxqB7TxgWfhqIr/YnGE4nagWzPchFJElTo=" 'Hello, world!'
    }
}
