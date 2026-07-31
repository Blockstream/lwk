//! Contains a software signer [`SwSigner`] and an [`AnySigner`] that can be a Jade or a Software signer.
//!
//! Signers should implement [`lwk_common::Signer`]

#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

mod software;
pub use crate::software::{sign_with_seckey, NewError, SignError, SwSigner};
#[cfg(feature = "silentpayments")]
mod silentpayments;

pub use bip39;
#[cfg(feature = "silentpayments")]
use lwk_common::silentpayments::{
    SilentPaymentAccount, SilentPaymentScanMaterial, SilentPaymentSigner,
};

use elements_miniscript::bitcoin::bip32::{self, DerivationPath, Fingerprint};
use elements_miniscript::bitcoin::sign_message::MessageSignature;
use elements_miniscript::elements::bitcoin::bip32::Xpub;
use elements_miniscript::elements::pset::PartiallySignedTransaction;
use lwk_common::Signer;

/// Possible errors when signing with [`AnySigner`]
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum SignerError {
    #[error(transparent)]
    Software(#[from] SignError),

    #[cfg(feature = "jade")]
    #[error(transparent)]
    JadeError(#[from] lwk_jade::error::Error),

    #[cfg(feature = "ledger")]
    #[error(transparent)]
    LedgerError(#[from] lwk_ledger::Error),

    #[error(transparent)]
    Bip32Error(#[from] bip32::Error),

    /// A hardware signer was asked for a silent-payment operation.
    ///
    /// This is a protocol gap, not an oversight: BIP-352 spending needs the device
    /// to combine its `b_spend` with a host-supplied tweak, and neither the Jade nor
    /// the Ledger protocol exposes such an operation today. Refusing loudly is the
    /// only honest answer — the alternative (deriving the key on the host) would
    /// defeat the entire point of using a hardware signer.
    #[cfg(feature = "silentpayments")]
    #[error("This signer does not support silent payments")]
    UnsupportedSilentPayments,
}

/// A signer that can be a software signer [`SwSigner`] or a [`lwk_jade::Jade`]
#[derive(Debug)]
pub enum AnySigner {
    /// A software signer [`SwSigner`]
    Software(SwSigner),

    /// A Jade signer [`lwk_jade::Jade`]
    #[cfg(feature = "jade")]
    Jade(lwk_jade::Jade, elements_miniscript::bitcoin::XKeyIdentifier),

    /// A Ledger signer [`lwk_ledger::Ledger`]
    #[cfg(feature = "ledger")]
    Ledger(
        lwk_ledger::Ledger<lwk_ledger::TransportTcp>,
        elements_miniscript::bitcoin::XKeyIdentifier,
    ),
}

impl Signer for AnySigner {
    type Error = SignerError;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        Signer::sign(&self, pset)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, Self::Error> {
        Signer::derive_xpub(&self, path)
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> Result<elements_miniscript::slip77::MasterBlindingKey, Self::Error> {
        Signer::slip77_master_blinding_key(&self)
    }

    fn fingerprint(&self) -> Result<Fingerprint, Self::Error> {
        Signer::fingerprint(&self)
    }

    fn sign_message(
        &self,
        message: &str,
        path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error> {
        Signer::sign_message(&self, message, path)
    }
}

/// Dispatches silent-payment scan-material export to the only signer that supports it.
///
/// Implemented on `AnySigner` rather than folded into [`Signer`] so signers with no
/// silent-payment support carry no dead state. Signing itself uses the single
/// [`Signer::sign`] operation; software signers recognize SP metadata there, while
/// hardware signers currently leave those unsupported inputs unsigned.
#[cfg(feature = "silentpayments")]
#[cfg_attr(docsrs, doc(cfg(feature = "silentpayments")))]
impl SilentPaymentSigner for AnySigner {
    fn silent_payment_scan_material(
        &self,
        account: SilentPaymentAccount,
    ) -> Result<SilentPaymentScanMaterial, Self::Error> {
        match self {
            AnySigner::Software(s) => Ok(s.silent_payment_scan_material(account)?),

            #[cfg(feature = "jade")]
            AnySigner::Jade(_, _) => Err(SignerError::UnsupportedSilentPayments),

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(_, _) => Err(SignerError::UnsupportedSilentPayments),
        }
    }
}

impl Signer for &AnySigner {
    type Error = SignerError;

    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error> {
        Ok(match self {
            AnySigner::Software(signer) => signer.sign(pset)?,

            #[cfg(feature = "jade")]
            AnySigner::Jade(signer, _) => signer.sign(pset)?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(signer, _) => signer.sign(pset)?,
        })
    }

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.derive_xpub(path)?,

            #[cfg(feature = "jade")]
            AnySigner::Jade(s, _) => s.derive_xpub(path)?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(s, _) => s.derive_xpub(path)?,
        })
    }

    fn slip77_master_blinding_key(
        &self,
    ) -> Result<elements_miniscript::slip77::MasterBlindingKey, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.slip77_master_blinding_key()?,

            #[cfg(feature = "jade")]
            AnySigner::Jade(s, _) => s.slip77_master_blinding_key()?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(s, _) => s.slip77_master_blinding_key()?,
        })
    }

    fn fingerprint(&self) -> Result<Fingerprint, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.fingerprint(),

            #[cfg(feature = "jade")]
            AnySigner::Jade(s, _) => s.fingerprint()?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(s, _) => s.fingerprint()?,
        })
    }

    fn sign_message(
        &self,
        message: &str,
        path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error> {
        Ok(match self {
            AnySigner::Software(s) => s.sign_message(message, path)?,

            #[cfg(feature = "jade")]
            AnySigner::Jade(s, _) => s.sign_message(message, path)?,

            #[cfg(feature = "ledger")]
            AnySigner::Ledger(s, _) => s.sign_message(message, path)?,
        })
    }
}
