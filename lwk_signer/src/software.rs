use bip39::Mnemonic;
use elements_miniscript::{
    bitcoin::{
        self,
        bip32::DerivationPath,
        hashes::{sha512, HashEngine, Hmac, HmacEngine},
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
#[allow(missing_docs)]
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

    #[error("Cannot derive BIP85 mnemonic (mnemonic/seed not available)")]
    Bip85MnemonicNotAvailable,

    #[error("BIP85 derivation failed: {0}")]
    Bip85Derivation(String),

    #[error("Taproot signing is not supported")]
    UnsupportedTaprootSigning,
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

    /// Derive a BIP85 mnemonic from the signer's mnemonic.
    ///
    /// This method uses BIP85 to deterministically derive a new mnemonic from the signer's
    /// master mnemonic. The derived mnemonic can be used for creating separate wallets
    /// while maintaining deterministic derivation from the original seed.
    ///
    /// # Arguments
    /// * `index` - The index for the derived mnemonic (0-based)
    /// * `word_count` - The number of words in the derived mnemonic (12 or 24)
    ///
    /// # Returns
    /// * `Ok(Mnemonic)` - The derived BIP85 mnemonic
    /// * `Err(SignError::Bip85MnemonicNotAvailable)` - If the signer was not initialized with a mnemonic
    /// * `Err(SignError::Bip85Derivation)` - If BIP85 derivation fails
    ///
    /// # Example
    /// ```rust
    /// use lwk_signer::SwSigner;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let signer = SwSigner::new("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about", false)?;
    /// let derived_mnemonic = signer.derive_bip85_mnemonic(0, 12)?;
    /// println!("Derived mnemonic: {}", derived_mnemonic);
    /// # Ok(())
    /// # }
    /// ```
    pub fn derive_bip85_mnemonic(
        &self,
        index: u32,
        word_count: u32,
    ) -> Result<Mnemonic, SignError> {
        // Check if we have a mnemonic available
        let _mnemonic = self
            .mnemonic
            .as_ref()
            .ok_or_else(|| SignError::Bip85MnemonicNotAvailable)?;

        // Convert the elements Xpriv to a standard bitcoin Xpriv for BIP85
        let seed = self
            .seed()
            .ok_or_else(|| SignError::Bip85MnemonicNotAvailable)?;

        // Create a standard bitcoin Xpriv from the seed
        let bitcoin_xprv = bitcoin::bip32::Xpriv::new_master(bitcoin::Network::Bitcoin, &seed)
            .map_err(|e| SignError::Bip85Derivation(format!("Failed to create Xpriv: {e}")))?;

        // Implement BIP85 derivation directly
        let derived_mnemonic = Self::bip85_derive_mnemonic(&bitcoin_xprv, index, word_count)?;

        Ok(derived_mnemonic)
    }

    /// Internal BIP85 mnemonic derivation implementation
    fn bip85_derive_mnemonic(
        xprv: &bitcoin::bip32::Xpriv,
        index: u32,
        word_count: u32,
    ) -> Result<Mnemonic, SignError> {
        // BIP85 constants
        const BIP85_PURPOSE: u32 = 83696968; // "m/83696968'"
        const BIP85_APPLICATION_39: u32 = 39; // BIP39 application
        const BIP85_ENTROPY_HMAC_KEY: &[u8] = b"bip-entropy-from-k";

        // Validate word count
        if word_count != 12 && word_count != 24 {
            return Err(SignError::Bip85Derivation(
                "Word count must be 12 or 24".to_string(),
            ));
        }

        // Derive the BIP85 path: m/83696968'/39'/language'/word_count'/index'
        let language = 0; // English
        let path = [
            BIP85_PURPOSE,        // hardened
            BIP85_APPLICATION_39, // hardened
            language,             // hardened
            word_count,           // hardened
            index,                // hardened
        ];

        // Convert path to DerivationPath
        let child_numbers: Result<Vec<_>, _> = path
            .iter()
            .map(|&i| bitcoin::bip32::ChildNumber::from_hardened_idx(i))
            .collect();
        let child_numbers = child_numbers
            .map_err(|e| SignError::Bip85Derivation(format!("Path creation failed: {e}")))?;
        let derivation_path = bitcoin::bip32::DerivationPath::from(child_numbers);

        // Derive the key using the path
        let derived_key = xprv
            .derive_priv(&bitcoin::secp256k1::Secp256k1::new(), &derivation_path)
            .map_err(|e| SignError::Bip85Derivation(format!("Key derivation failed: {e}")))?;

        // HMAC-SHA512 the derived private key with the fixed BIP85 key
        let mut hmac_engine = HmacEngine::<sha512::Hash>::new(BIP85_ENTROPY_HMAC_KEY);

        // Use the private key bytes (excluding the first byte which is the network prefix)
        let priv_key_bytes = &derived_key.private_key.secret_bytes();
        hmac_engine.input(priv_key_bytes);
        let hmac_result = Hmac::<sha512::Hash>::from_engine(hmac_engine);

        // Extract the appropriate amount of entropy based on word count
        let entropy_len = if word_count == 12 { 16 } else { 32 }; // 128 or 256 bits
        let entropy = &hmac_result.to_byte_array()[..entropy_len];

        // Create mnemonic from entropy
        let mnemonic = Mnemonic::from_entropy(entropy)
            .map_err(|e| SignError::Bip85Derivation(format!("Mnemonic creation failed: {e}")))?;

        Ok(mnemonic)
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
        for (i, inp) in pset.inputs().iter().enumerate() {
            if inp.tap_internal_key.is_some() {
                return Err(SignError::UnsupportedTaprootSigning);
            }
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
        assert_eq!(format!("{signer:?}"), "Signer(73c5da0a)");
        assert_eq!(
            "mnemonic has an invalid word count: 1. Word count must be 12, 15, 18, 21, or 24",
            SwSigner::new("bad", false).expect_err("test").to_string()
        );
        assert_eq!(
            lwk_test_util::TEST_MNEMONIC_XPUB,
            &signer.xpub().to_string()
        );

        let slip77 = signer.slip77_master_blinding_key().unwrap();
        assert_eq!(slip77.as_bytes().to_hex(), format!("{slip77}"));
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

    #[test]
    fn test_bip85_mnemonic_derivation() {
        // Test with a known mnemonic
        let signer = SwSigner::new(lwk_test_util::TEST_MNEMONIC, false).unwrap();

        // Derive a 12-word mnemonic at index 0
        let derived_0_12 = signer.derive_bip85_mnemonic(0, 12).unwrap();
        assert_eq!(derived_0_12.word_count(), 12);

        // Derive a 24-word mnemonic at index 0
        let derived_0_24 = signer.derive_bip85_mnemonic(0, 24).unwrap();
        assert_eq!(derived_0_24.word_count(), 24);

        // Derive different mnemonics at different indices
        let derived_1_12 = signer.derive_bip85_mnemonic(1, 12).unwrap();
        let derived_2_12 = signer.derive_bip85_mnemonic(2, 12).unwrap();
        let derived_3_12 = signer.derive_bip85_mnemonic(3, 12).unwrap();

        // check derived mnemonics
        assert_eq!(signer.mnemonic().unwrap().to_string(), "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        assert_eq!(
            derived_0_12.to_string(),
            "prosper short ramp prepare exchange stove life snack client enough purpose fold"
        );
        assert_eq!(derived_0_24.to_string(), "stick exact spice sock filter ginger museum horse kit multiply manual wear grief demand derive alert quiz fault december lava picture immune decade jaguar");
        assert_eq!(
            derived_1_12.to_string(),
            "sing slogan bar group gauge sphere rescue fossil loyal vital model desert"
        );
        assert_eq!(
            derived_2_12.to_string(),
            "comfort onion auto dizzy upgrade mutual banner announce section poet point pudding"
        );
        assert_eq!(
            derived_3_12.to_string(),
            "tuna mention protect shrimp mushroom access cat cattle license bind equip trial"
        );

        // derived mnemonics can be checked using bip85-cli (pip install bip85-cli)
    }

    #[test]
    fn test_bip85_mnemonic_derivation_testvectors() {
        // Test with a known mnemonic from jade testvectors
        let mnemonic = "fish inner face ginger orchard permit useful method fence kidney chuckle party favorite sunset draw limb science crane oval letter slot invite sadness banana";
        let signer = SwSigner::new(mnemonic, false).unwrap();

        // Derive menmonics from testvectors
        let derived_0_12 = signer.derive_bip85_mnemonic(0, 12).unwrap();
        let derived_12_12 = signer.derive_bip85_mnemonic(12, 12).unwrap();
        let derived_100_12 = signer.derive_bip85_mnemonic(100, 12).unwrap();
        let derived_65535_12 = signer.derive_bip85_mnemonic(65535, 12).unwrap();
        let derived_0_24 = signer.derive_bip85_mnemonic(0, 24).unwrap();
        let derived_24_24 = signer.derive_bip85_mnemonic(24, 24).unwrap();
        let derived_1024_24 = signer.derive_bip85_mnemonic(1024, 24).unwrap();
        let derived_65535_24 = signer.derive_bip85_mnemonic(65535, 24).unwrap();

        // check derived mnemonics
        assert_eq!(
            derived_0_12.to_string(),
            "elephant this puppy lucky fatigue skate aerobic emotion peanut outer clinic casino"
        );
        assert_eq!(
            derived_12_12.to_string(),
            "prevent marriage menu outside total tone prison few sword coffee print salad"
        );
        assert_eq!(
            derived_100_12.to_string(),
            "lottery divert goat drink tackle picture youth text stem marriage call tip"
        );
        assert_eq!(
            derived_65535_12.to_string(),
            "curtain angle fatigue siren involve bleak detail frame name spare size cycle"
        );
        assert_eq!(
            derived_0_24.to_string(),
            "certain act palace ball plug they divide fold climb hand tuition inside choose sponsor grass scheme choose split top twenty always vendor fit thank"
        );
        assert_eq!(
            derived_24_24.to_string(),
            "flip meat face wood hammer crack fat topple admit canvas bid capital leopard angry fan gate domain exile patient recipe nut honey resist inner"
        );
        assert_eq!(
            derived_1024_24.to_string(),
            "phone goat wheel unique local maximum sand reflect scissors one have spin weasel dignity antenna acid pulp increase fitness typical bacon strike spy festival"
        );
        assert_eq!(
            derived_65535_24.to_string(),
            "humble museum grab fitness wrap window front job quarter update rich grape gap daring blame cricket traffic sad trade easily genius boost lumber rhythm"
        );

        // derived mnemonics from jade testvectors https://github.com/Blockstream/Jade/blob/master/test_jade.py
    }

    #[test]
    fn test_bip85_mnemonic_derivation_without_mnemonic() {
        // Test that BIP85 derivation fails when signer was created from xprv
        use std::str::FromStr;
        let xprv = Xpriv::from_str("tprv8bxtvyWEZW9M4n8ByZVSG2NNP4aeiRdhDZXNEv1eVNtrhLLnc6vJ1nf9DN5cHAoxMwqRR1CD6YXBvw2GncSojF8DknPnQVMgbpkjnKHkrGY").unwrap();
        let signer = SwSigner::from_xprv(xprv);

        // Should fail because no mnemonic is available
        let result = signer.derive_bip85_mnemonic(0, 12);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SignError::Bip85MnemonicNotAvailable
        ));
    }

    #[test]
    #[allow(unused)]
    fn test_snippet() -> Result<(), Box<dyn std::error::Error>> {
        // ANCHOR: test_bip85_derivation
        // Load mnemonic
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        // Create signer
        let is_mainnet = false;
        let signer = SwSigner::new(mnemonic, is_mainnet)?;

        // Derive menmonics
        let derived_0_12 = signer.derive_bip85_mnemonic(0, 12)?;
        let derived_0_24 = signer.derive_bip85_mnemonic(0, 24)?;
        let derived_1_12 = signer.derive_bip85_mnemonic(1, 12)?;
        // ANCHOR_END: test_bip85_derivation

        Ok(())
    }
}
