use std::sync::Arc;

use lwk_common::Bip;
use lwk_simplicity::error::WalletAbiError;
use lwk_simplicity::wallet_abi::schema::SignerMeta;
use lwk_wollet::bitcoin::bip32::{ChildNumber, DerivationPath, Fingerprint};
use lwk_wollet::bitcoin::PublicKey as BitcoinPublicKey;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{Address as ElementsAddress, TxOut as ElementsTxOut, TxOutSecrets};
use lwk_wollet::secp256k1::schnorr::Signature;
use lwk_wollet::secp256k1::{Message, XOnlyPublicKey as BitcoinXOnlyPublicKey};
use lwk_wollet::EC;
use tracing::{error, info};

use crate::types::{PublicKey, XOnlyPublicKey};
use crate::{Address, LwkError, Network, Pset, TxOut, TxOutSecrets as BindingsTxOutSecrets};

const HARDENED_BIT: u32 = 0x8000_0000;

/// BIP branch selector used by wallet-abi signer callbacks.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletAbiBip {
    /// BIP49 branch.
    Bip49,
    /// BIP84 branch.
    Bip84,
    /// BIP87 branch.
    Bip87,
}

impl From<Bip> for WalletAbiBip {
    fn from(value: Bip) -> Self {
        match value {
            Bip::Bip49 => Self::Bip49,
            Bip::Bip84 => Self::Bip84,
            Bip::Bip87 => Self::Bip87,
        }
    }
}

/// Callback interface used by foreign code to provide runtime signer capabilities.
#[uniffi::export(with_foreign)]
pub trait WalletAbiSignerCallbacks: Send + Sync {
    /// Return active signer network.
    fn get_network(&self) -> Arc<Network>;

    /// Return signer receive address used for wallet outputs/change.
    fn get_signer_receive_address(&self) -> Result<Arc<Address>, LwkError>;

    /// Return signer master fingerprint encoded as big-endian `u32`.
    fn fingerprint(&self) -> u32;

    /// Return base derivation path encoded as BIP32 child numbers.
    ///
    /// Each item uses hardened-bit encoding (`0x8000_0000`) for hardened children.
    fn get_derivation_path(&self, bip: WalletAbiBip) -> Vec<u32>;

    /// Return compressed public key for a given BIP32 derivation path.
    ///
    /// Path encoding follows the same hardened-bit convention as `get_derivation_path`.
    fn get_pubkey_by_derivation_path(
        &self,
        derivation_path: Vec<u32>,
    ) -> Result<Arc<PublicKey>, LwkError>;

    /// Return signer x-only public key used in runtime witness directives.
    fn get_raw_signing_x_only_pubkey(&self) -> Result<Arc<XOnlyPublicKey>, LwkError>;

    /// Unblind one output and return unblinded secrets.
    fn unblind(&self, tx_out: Arc<TxOut>) -> Result<Arc<BindingsTxOutSecrets>, LwkError>;

    /// Sign PSET inputs and return signed PSET.
    fn sign_pst(&self, pst: Arc<Pset>) -> Result<Arc<Pset>, LwkError>;

    /// Create one Schnorr signature for a 32-byte message digest.
    fn sign_schnorr(&self, message: Vec<u8>) -> Result<Vec<u8>, LwkError>;
}

/// Bridge object adapting foreign signer callbacks to runtime `SignerMeta`.
#[derive(uniffi::Object)]
pub struct SignerMetaLink {
    pub(crate) inner: Arc<dyn WalletAbiSignerCallbacks>,
}

#[uniffi::export]
impl SignerMetaLink {
    /// Create a signer bridge from foreign callback implementation.
    #[uniffi::constructor]
    pub fn new(callbacks: Arc<dyn WalletAbiSignerCallbacks>) -> Self {
        Self { inner: callbacks }
    }
}

fn signer_callback_error(context: &str, error: LwkError) -> WalletAbiError {
    WalletAbiError::InvalidSignerConfig(format!(
        "wallet-abi signer callback '{context}' failed: {error:?}"
    ))
}

fn signer_conversion_error(context: &str, detail: impl std::fmt::Display) -> WalletAbiError {
    WalletAbiError::InvalidSignerConfig(format!(
        "wallet-abi signer conversion '{context}' failed: {detail}"
    ))
}

fn encode_derivation_path(path: &DerivationPath) -> Vec<u32> {
    path.into_iter()
        .map(|child| match child {
            ChildNumber::Normal { index } => *index,
            ChildNumber::Hardened { index } => *index | HARDENED_BIT,
        })
        .collect()
}

fn decode_derivation_path(path: &[u32]) -> DerivationPath {
    let mut children = Vec::with_capacity(path.len());
    for raw in path {
        let index = raw & !HARDENED_BIT;
        let child = if (raw & HARDENED_BIT) != 0 {
            ChildNumber::from_hardened_idx(index).expect("masked index always valid")
        } else {
            ChildNumber::from_normal_idx(index).expect("masked index always valid")
        };
        children.push(child);
    }
    DerivationPath::from(children)
}

fn format_encoded_path(path: &[u32]) -> String {
    if path.is_empty() {
        return "m".to_string();
    }

    let path_tail = path
        .iter()
        .map(|raw| {
            let index = raw & !HARDENED_BIT;
            if (raw & HARDENED_BIT) != 0 {
                format!("{index}h")
            } else {
                index.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/");

    format!("m/{path_tail}")
}

fn log_pset_signature_summary(stage: &str, pst: &PartiallySignedTransaction) {
    for (input_index, input) in pst.inputs().iter().enumerate() {
        let expected_pubkeys = input
            .bip32_derivation
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let partial_sig_pubkeys = input
            .partial_sigs
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        info!(
            target: "wallet_abi",
            stage,
            input_index,
            bip32_derivations = input.bip32_derivation.len(),
            partial_sigs = input.partial_sigs.len(),
            expected_pubkeys = %expected_pubkeys.join(","),
            partial_sig_pubkeys = %partial_sig_pubkeys.join(","),
            "wallet-abi signer pset signature summary"
        );
    }
}

impl SignerMeta for SignerMetaLink {
    type Error = WalletAbiError;

    fn get_network(&self) -> lwk_common::Network {
        self.inner.get_network().as_ref().into()
    }

    fn get_signer_receive_address(&self) -> Result<ElementsAddress, Self::Error> {
        let address = self
            .inner
            .get_signer_receive_address()
            .map_err(|error| signer_callback_error("get_signer_receive_address", error))?;
        Ok(address.as_ref().into())
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from(self.inner.fingerprint().to_be_bytes())
    }

    fn get_derivation_path(&self, bip: Bip) -> DerivationPath {
        let encoded = self.inner.get_derivation_path(bip.into());
        info!(
            target: "wallet_abi",
            bip = ?bip,
            encoded_path = %format_encoded_path(&encoded),
            "wallet-abi signer callback returned derivation path"
        );
        decode_derivation_path(&encoded)
    }

    fn get_pubkey_by_derivation_path(
        &self,
        derivation_path: &DerivationPath,
    ) -> Result<BitcoinPublicKey, Self::Error> {
        let path = encode_derivation_path(derivation_path);
        info!(
            target: "wallet_abi",
            requested_path = %format_encoded_path(&path),
            "wallet-abi signer callback get_pubkey_by_derivation_path request"
        );
        let key = self
            .inner
            .get_pubkey_by_derivation_path(path)
            .map_err(|error| signer_callback_error("get_pubkey_by_derivation_path", error))?;
        info!(
            target: "wallet_abi",
            requested_path = %derivation_path,
            pubkey = %key,
            "wallet-abi signer callback get_pubkey_by_derivation_path response"
        );
        Ok(key.as_ref().into())
    }

    fn get_raw_signing_x_only_pubkey(&self) -> Result<BitcoinXOnlyPublicKey, Self::Error> {
        let key = self
            .inner
            .get_raw_signing_x_only_pubkey()
            .map_err(|error| signer_callback_error("get_raw_signing_x_only_pubkey", error))?;
        Ok((*key.as_ref()).into())
    }

    fn unblind(&self, tx_out: &ElementsTxOut) -> Result<TxOutSecrets, Self::Error> {
        let tx_out_bindings: TxOut = tx_out.clone().into();
        let tx_out_bindings = Arc::new(tx_out_bindings);
        let secrets = self
            .inner
            .unblind(tx_out_bindings)
            .map_err(|error| signer_callback_error("unblind", error))?;

        Ok(secrets.as_ref().into())
    }

    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
        log_pset_signature_summary("before_sign_pst", pst);

        let pst_bindings = Arc::new(Pset::from(pst.clone()));
        let signed = self.inner.sign_pst(pst_bindings).map_err(|error| {
            error!(
                target: "wallet_abi",
                reason = %format!("{error:?}"),
                "wallet-abi signer callback sign_pst failed"
            );
            signer_callback_error("sign_pst", error)
        })?;
        *pst = signed.as_ref().inner();
        log_pset_signature_summary("after_sign_pst", pst);
        Ok(())
    }

    fn sign_schnorr(
        &self,
        message: Message,
        xonly_public_key: BitcoinXOnlyPublicKey,
    ) -> Result<Signature, Self::Error> {
        info!(
            target: "wallet_abi",
            message_len = message.as_ref().len(),
            "wallet-abi signer callback sign_schnorr request"
        );
        let signature = self
            .inner
            .sign_schnorr(message.as_ref().to_vec())
            .map_err(|error| signer_callback_error("sign_schnorr", error))?;
        if signature.len() != 64 {
            return Err(signer_conversion_error(
                "sign_schnorr",
                format!("expected 64-byte signature, got {} bytes", signature.len()),
            ));
        }
        let signature = Signature::from_slice(&signature)
            .map_err(|error| signer_conversion_error("sign_schnorr", error))?;
        let x_only_pubkey = self
            .inner
            .get_raw_signing_x_only_pubkey()
            .map_err(|error| signer_callback_error("get_raw_signing_x_only_pubkey", error))?;
        let x_only_pubkey: BitcoinXOnlyPublicKey = (*x_only_pubkey.as_ref()).into();

        if x_only_pubkey != xonly_public_key {
            return Err(signer_conversion_error(
                "sign_schnorr key mismatch",
                format!(
                    "callback key {x_only_pubkey} does not match requested key {xonly_public_key}"
                ),
            ));
        }

        if let Err(error) = EC.verify_schnorr(&signature, &message, &xonly_public_key) {
            error!(
                target: "wallet_abi",
                x_only_pubkey = %xonly_public_key,
                reason = %error,
                "wallet-abi signer callback sign_schnorr returned signature that failed verification"
            );
            return Err(signer_conversion_error("sign_schnorr verification", error));
        }

        info!(
            target: "wallet_abi",
            x_only_pubkey = %xonly_public_key,
            "wallet-abi signer callback sign_schnorr verification passed"
        );
        Ok(signature)
    }
}
