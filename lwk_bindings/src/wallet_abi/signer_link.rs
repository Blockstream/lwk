use std::sync::Arc;
use std::{fmt, str::FromStr};

use crate::{LwkError, Pset, Signer, WalletAbiSignerContext, XOnlyPublicKey};

use elements::bitcoin::bip32::DerivationPath;
use lwk_simplicity::wallet_abi::KeyStoreMeta;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::secp256k1::{
    schnorr::Signature, Keypair, Message, XOnlyPublicKey as SecpXOnlyPublicKey,
};
use lwk_wollet::EC;

/// Callback interface used by foreign code to provide runtime signer capabilities.
#[uniffi::export(with_foreign)]
pub trait WalletAbiSignerCallbacks: Send + Sync {
    /// Return signer x-only public key used in runtime witness directives.
    fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError>;

    /// Sign PSET inputs and return the updated PSET.
    fn sign_pst(&self, pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError>;

    /// Create one Schnorr signature for a 32-byte message digest.
    fn sign_schnorr(&self, message: Vec<u8>) -> Result<Vec<u8>, LwkError>;
}

/// Error type for the signer callback bridge.
#[derive(thiserror::Error, Debug)]
pub enum SignerMetaLinkError {
    /// Error returned by the foreign callback implementation.
    #[error("{0}")]
    Foreign(String),
    /// The foreign callback returned bytes that are not a valid Schnorr signature.
    #[error("foreign signer returned invalid Schnorr signature bytes")]
    InvalidSignatureBytes,
    /// The returned Schnorr signature does not verify against the requested x-only public key.
    #[error("foreign signer returned a signature that does not match the requested x-only public key")]
    SignatureVerificationFailed,
}

/// Bridge object adapting foreign signer implementations to runtime `KeyStoreMeta`.
#[derive(uniffi::Object)]
pub struct SignerMetaLink {
    inner: Arc<dyn WalletAbiSignerCallbacks>,
}

#[uniffi::export]
impl SignerMetaLink {
    /// Create a signer bridge from foreign callback implementation.
    #[uniffi::constructor]
    pub fn new(callbacks: Arc<dyn WalletAbiSignerCallbacks>) -> Self {
        Self { inner: callbacks }
    }

    /// Build a Rust-owned wallet-abi signer bridge from a software signer.
    #[uniffi::constructor(name = "from_software_signer")]
    pub fn from_software_signer(
        signer: Arc<Signer>,
        context: WalletAbiSignerContext,
    ) -> Result<Self, LwkError> {
        let derivation_path = software_signer_derivation_path(&context)?;
        let derived_xprv = signer.inner.derive_xprv(&derivation_path)?;
        let keypair = Keypair::from_secret_key(&EC, &derived_xprv.private_key);

        Ok(Self::new(Arc::new(SoftwareSignerCallbacks {
            signer,
            keypair,
            xonly_public_key: XOnlyPublicKey::from_keypair(&keypair),
        })))
    }
}

impl KeyStoreMeta for SignerMetaLink {
    type Error = SignerMetaLinkError;

    fn get_raw_signing_x_only_pubkey(&self) -> Result<SecpXOnlyPublicKey, Self::Error> {
        self.inner
            .get_raw_signing_x_only_pubkey()
            .map(|key| (*key).into())
            .map_err(|error| SignerMetaLinkError::Foreign(format!("{error:?}")))
    }

    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
        let signed = self
            .inner
            .sign_pst(Arc::new(pst.clone().into()))
            .map_err(|error| SignerMetaLinkError::Foreign(format!("{error:?}")))?;
        *pst = signed.inner();
        Ok(())
    }

    fn sign_schnorr(
        &self,
        message: Message,
        xonly_public_key: SecpXOnlyPublicKey,
    ) -> Result<Signature, Self::Error> {
        let signature_bytes = self
            .inner
            .sign_schnorr(message.as_ref().to_vec())
            .map_err(|error| SignerMetaLinkError::Foreign(format!("{error:?}")))?;
        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|_| SignerMetaLinkError::InvalidSignatureBytes)?;
        EC.verify_schnorr(&signature, &message, &xonly_public_key)
            .map_err(|_| SignerMetaLinkError::SignatureVerificationFailed)?;
        Ok(signature)
    }
}

fn software_signer_derivation_path(
    context: &WalletAbiSignerContext,
) -> Result<DerivationPath, LwkError> {
    let coin_type = if context.network.is_mainnet() { 1776 } else { 1 };
    DerivationPath::from_str(&format!(
        "m/86h/{coin_type}h/{}h/0/0",
        context.account_index
    ))
    .map_err(Into::into)
}

struct SoftwareSignerCallbacks {
    signer: Arc<Signer>,
    keypair: Keypair,
    xonly_public_key: Arc<XOnlyPublicKey>,
}

impl fmt::Debug for SoftwareSignerCallbacks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SoftwareSignerCallbacks")
    }
}

impl WalletAbiSignerCallbacks for SoftwareSignerCallbacks {
    fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
        Ok(self.xonly_public_key.clone())
    }

    fn sign_pst(&self, pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError> {
        self.signer.sign(pst.as_ref())
    }

    fn sign_schnorr(&self, message: Vec<u8>) -> Result<Vec<u8>, LwkError> {
        let message = Message::from_digest_slice(&message)?;
        Ok(self.keypair.sign_schnorr(message).serialize().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use elements::bitcoin::secp256k1::{Keypair, Message, Secp256k1, SecretKey};
    use lwk_common::Signer as _;
    use lwk_wollet::ElementsNetwork;

    use super::*;
    use crate::{Mnemonic, Network};

    struct TestSignerCallbacks {
        keypair: Keypair,
        signed_pset: Arc<Pset>,
    }

    impl WalletAbiSignerCallbacks for TestSignerCallbacks {
        fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError> {
            Ok(XOnlyPublicKey::from_keypair(&self.keypair))
        }

        fn sign_pst(&self, _pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError> {
            Ok(self.signed_pset.clone())
        }

        fn sign_schnorr(&self, message: Vec<u8>) -> Result<Vec<u8>, LwkError> {
            let message = Message::from_digest_slice(&message)?;
            let signature = EC.sign_schnorr(&message, &self.keypair);
            Ok(signature.serialize().to_vec())
        }
    }

    fn test_signer_link() -> (SignerMetaLink, Keypair) {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&[0x11; 32]).expect("secret key");
        let keypair = Keypair::from_secret_key(&secp, &secret_key);
        let signed_pset = lwk_test_util::psets_to_combine().1[1].clone();
        let callbacks = Arc::new(TestSignerCallbacks {
            keypair,
            signed_pset: Arc::new(Pset::from(signed_pset)),
        });
        (SignerMetaLink::new(callbacks), keypair)
    }

    #[test]
    fn signer_meta_link_get_raw_signing_x_only_pubkey() {
        let (signer_link, keypair) = test_signer_link();
        let expected = keypair.x_only_public_key().0;

        let actual = signer_link
            .get_raw_signing_x_only_pubkey()
            .expect("x-only public key");

        assert_eq!(actual, expected);
    }

    #[test]
    fn signer_meta_link_sign_pst() {
        let (signer_link, _keypair) = test_signer_link();
        let mut pset = lwk_test_util::psets_to_combine().1[0].clone();
        let expected = lwk_test_util::psets_to_combine().1[1].clone().to_string();

        signer_link.sign_pst(&mut pset).expect("sign pset");

        assert_eq!(pset.to_string(), expected);
    }

    #[test]
    fn signer_meta_link_sign_schnorr() {
        let (signer_link, keypair) = test_signer_link();
        let message = Message::from_digest([0x22; 32]);
        let xonly_public_key = keypair.x_only_public_key().0;

        let signature = signer_link
            .sign_schnorr(message, xonly_public_key)
            .expect("schnorr signature");

        assert!(EC
            .verify_schnorr(&signature, &message, &xonly_public_key)
            .is_ok());
    }

    #[test]
    fn signer_meta_link_from_software_signer_uses_context_key() {
        let mnemonic = Mnemonic::new(lwk_test_util::TEST_MNEMONIC).expect("mnemonic");
        let network: Arc<Network> = Arc::new(ElementsNetwork::default_regtest().into());
        let signer = Signer::new(&mnemonic, &network).expect("signer");
        let context = WalletAbiSignerContext {
            network: network.clone(),
            account_index: 7,
        };
        let signer_link =
            SignerMetaLink::from_software_signer(signer.clone(), context.clone()).expect("link");
        let derivation_path = software_signer_derivation_path(&context).expect("path");
        let expected_xpub = signer
            .inner
            .derive_xpub(&derivation_path)
            .expect("derive xpub");
        let expected_xonly = expected_xpub.public_key.x_only_public_key().0;

        assert_eq!(
            signer_link
                .get_raw_signing_x_only_pubkey()
                .expect("x-only public key"),
            expected_xonly
        );

        let message = Message::from_digest([0x33; 32]);
        let signature = signer_link
            .sign_schnorr(message, expected_xonly)
            .expect("signature");

        assert!(EC
            .verify_schnorr(&signature, &message, &expected_xonly)
            .is_ok());

        let pset = Pset::new(include_str!("../../../lwk_jade/test_data/pset_to_be_signed.base64"))
            .expect("pset");
        let mut callback_pset = pset.inner();
        signer_link.sign_pst(&mut callback_pset).expect("signed pset");

        let expected_pset = signer.sign(pset.as_ref()).expect("expected signed pset");
        assert_eq!(callback_pset.to_string(), expected_pset.to_string());
    }
}
