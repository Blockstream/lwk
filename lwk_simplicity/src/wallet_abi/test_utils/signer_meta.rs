use lwk_common::{Bip, Network, Signer};
use lwk_signer::SwSigner;
use lwk_wollet::bitcoin::bip32::{DerivationPath, Fingerprint};
use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{Address, TxOut, TxOutSecrets};
use lwk_wollet::secp256k1::schnorr::Signature;
use lwk_wollet::secp256k1::{Keypair, Message, XOnlyPublicKey};
use lwk_wollet::{WolletDescriptor, EC};
use simplicityhl::simplicity::ToXOnlyPubkey;
use std::str::FromStr;

use crate::error::WalletAbiError;
use crate::wallet_abi::schema::SignerMeta;

/// Test implementation of [`SignerMeta`] backed by [`SwSigner`].
#[derive(Clone)]
pub struct TestSignerMeta {
    signer: SwSigner,
    network: Network,
    descriptor: WolletDescriptor,
}

impl TestSignerMeta {
    /// Create test signer metadata from an existing software signer.
    pub fn from_signer(signer: SwSigner, network: Network) -> Result<Self, WalletAbiError> {
        let descriptor = descriptor_from_signer(&signer)?;

        Ok(Self {
            signer,
            network,
            descriptor,
        })
    }

    /// Create test signer metadata from a mnemonic.
    pub fn from_mnemonic(mnemonic: &str, network: Network) -> Result<Self, WalletAbiError> {
        let signer = SwSigner::new(mnemonic, network.is_mainnet()).map_err(|error| {
            WalletAbiError::InvalidSignerConfig(format!(
                "failed to create software signer from mnemonic: {error}"
            ))
        })?;

        Self::from_signer(signer, network)
    }

    fn signing_keypair(&self) -> Result<Keypair, WalletAbiError> {
        let xprv = self
            .signer
            .derive_xprv(&self.get_derivation_path(Bip::Bip87))
            .map_err(|error| {
                WalletAbiError::InvalidSignerConfig(format!(
                    "failed to derive BIP87 signing key: {error}"
                ))
            })?;

        Ok(xprv.to_keypair(&EC))
    }

    pub fn network(&self) -> Network {
        self.network
    }

    pub fn signer_receive_address(&self) -> Result<Address, WalletAbiError> {
        self.get_signer_receive_address()
    }

    pub fn signer_x_only_public_key(&self) -> Result<XOnlyPublicKey, WalletAbiError> {
        self.get_raw_signing_x_only_pubkey()
    }

    /// Return the parsed descriptor derived from this signer.
    pub fn descriptor(&self) -> &WolletDescriptor {
        &self.descriptor
    }
}

impl SignerMeta for TestSignerMeta {
    type Error = WalletAbiError;

    fn get_network(&self) -> Network {
        self.network
    }

    fn get_signer_receive_address(&self) -> Result<Address, Self::Error> {
        self.descriptor
            .address(1, self.network.address_params())
            .map_err(|error| {
                WalletAbiError::InvalidSignerConfig(format!(
                    "failed to derive signer receive address: {error}"
                ))
            })
    }

    fn fingerprint(&self) -> Fingerprint {
        self.signer.fingerprint()
    }

    fn get_derivation_path(&self, bip: Bip) -> DerivationPath {
        let coin_type = if self.network.is_mainnet() { 1776 } else { 1 };
        let purpose = match bip {
            Bip::Bip84 => 84,
            Bip::Bip49 => 49,
            Bip::Bip87 => 87,
        };

        DerivationPath::from_str(&format!("m/{purpose}h/{coin_type}h/0h"))
            .expect("static derivation path")
    }

    fn get_pubkey_by_derivation_path(
        &self,
        derivation_path: &DerivationPath,
    ) -> Result<PublicKey, Self::Error> {
        self.signer
            .derive_xpub(derivation_path)
            .map(|xpub| xpub.public_key.into())
            .map_err(|error| {
                WalletAbiError::InvalidSignerConfig(format!(
                    "failed to derive public key at path {derivation_path}: {error}"
                ))
            })
    }

    fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
        Ok(self.signing_keypair()?.x_only_public_key().0)
    }

    fn unblind(&self, tx_out: &TxOut) -> Result<TxOutSecrets, Self::Error> {
        let master_blinding_key = self.signer.slip77_master_blinding_key().map_err(|error| {
            WalletAbiError::InvalidSignerConfig(format!(
                "failed to derive signer slip77 master blinding key: {error}"
            ))
        })?;
        let blinding_private_key = master_blinding_key.blinding_private_key(&tx_out.script_pubkey);
        let secrets = tx_out.unblind(&EC, blinding_private_key).map_err(|error| {
            WalletAbiError::InvalidRequest(format!("failed to unblind transaction output: {error}"))
        })?;

        Ok(secrets)
    }

    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
        self.signer.sign(pst).map_err(|error| {
            WalletAbiError::InvalidFinalizationSteps(format!("failed to sign PSET inputs: {error}"))
        })?;

        Ok(())
    }

    fn sign_schnorr(
        &self,
        message: Message,
        xonly_public_key: XOnlyPublicKey,
    ) -> Result<Signature, Self::Error> {
        let keypair = self.signing_keypair()?;

        if keypair.public_key().to_x_only_pubkey() != xonly_public_key {
            return Err(WalletAbiError::InvalidFinalizationSteps(format!(
                "keypair identity used for signing {} differs from the key expected by the caller: {xonly_public_key}", keypair.public_key().to_x_only_pubkey()
            )));
        }

        Ok(EC.sign_schnorr(&message, &keypair))
    }
}

fn descriptor_from_signer(signer: &SwSigner) -> Result<WolletDescriptor, WalletAbiError> {
    let descriptor = signer
        .wpkh_slip77_descriptor()
        .map_err(WalletAbiError::InvalidSignerConfig)?;

    WolletDescriptor::from_str(&descriptor).map_err(|error| {
        WalletAbiError::InvalidSignerConfig(format!(
            "failed to parse signer descriptor '{descriptor}': {error}"
        ))
    })
}
