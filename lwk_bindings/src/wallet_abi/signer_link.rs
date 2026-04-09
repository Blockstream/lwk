use std::sync::Arc;

use crate::{LwkError, Pset, XOnlyPublicKey};

use lwk_simplicity::wallet_abi::KeyStoreMeta;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::secp256k1::{schnorr::Signature, Message, XOnlyPublicKey as SecpXOnlyPublicKey};
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use elements::bitcoin::secp256k1::{Keypair, Message, Secp256k1, SecretKey};

    use super::*;

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
}
