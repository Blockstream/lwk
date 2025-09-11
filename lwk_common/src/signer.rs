use std::str::FromStr;

use elements::{
    bitcoin::{
        self,
        bip32::{ChildNumber, DerivationPath, Fingerprint, Xpub},
        sign_message::MessageSignature,
        XKeyIdentifier,
    },
    pset::PartiallySignedTransaction,
};
use elements_miniscript::slip77::MasterBlindingKey;

use crate::descriptor::Bip;

/// A trait defining methods of signers, providing blanket implementations for some methods.
pub trait Signer {
    type Error: std::fmt::Debug;

    /// Try to sign the given pset, mutating it in place.
    /// returns how many signatures were added or overwritten
    fn sign(&self, pset: &mut PartiallySignedTransaction) -> Result<u32, Self::Error>;

    /// Derive an xpub from the master, path can contains hardened derivations
    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, Self::Error>;

    /// Return the slip77 master blinding key
    fn slip77_master_blinding_key(&self) -> Result<MasterBlindingKey, Self::Error>;

    /// Return the master xpub of the signer
    fn xpub(&self) -> Result<Xpub, Self::Error> {
        self.derive_xpub(&DerivationPath::master())
    }

    /// Return the full identifier of the signer
    fn identifier(&self) -> Result<XKeyIdentifier, Self::Error> {
        Ok(self.xpub()?.identifier())
    }

    /// Return the fingerprint of the signer (4 bytes)
    fn fingerprint(&self) -> Result<Fingerprint, Self::Error> {
        Ok(self.xpub()?.fingerprint())
    }

    /// Return keyorigin and xpub, like "[73c5da0a/84h/1h/0h]tpub..."
    fn keyorigin_xpub(&self, bip: Bip, is_mainnet: bool) -> Result<String, Self::Error> {
        let coin_type = if is_mainnet { 1776 } else { 1 };
        let path = match bip {
            Bip::Bip84 => format!("84h/{coin_type}h/0h"),
            Bip::Bip49 => format!("49h/{coin_type}h/0h"),
            Bip::Bip87 => format!("87h/{coin_type}h/0h"),
        };

        let fingerprint = self.fingerprint()?;
        let xpub =
            self.derive_xpub(&DerivationPath::from_str(&format!("m/{path}")).expect("static"))?; // TODO avoid string use ChildNumber directly
        let keyorigin_xpub = format!("[{fingerprint}/{path}]{xpub}");
        Ok(keyorigin_xpub)
    }

    fn is_mainnet(&self) -> Result<bool, Self::Error> {
        let xpub = match self.xpub() {
            Ok(xpub) => xpub,
            Err(_) => {
                // We are probably on a Ledger that won't return the master xpub
                let path = [
                    ChildNumber::from_hardened_idx(44).expect("static"),
                    ChildNumber::from_hardened_idx(1).expect("static"), // TODO: work on  mainnet?
                    ChildNumber::from_hardened_idx(0).expect("static"),
                ];
                self.derive_xpub(&DerivationPath::from_iter(path))?
            }
        };
        Ok(xpub.network == bitcoin::NetworkKind::Main)
    }

    fn wpkh_slip77_descriptor(&self) -> Result<String, String> {
        crate::singlesig_desc(
            self,
            crate::Singlesig::Wpkh,
            crate::DescriptorBlindingKey::Slip77,
        )
    }

    /// Sign a message using Bitcoin’s message signing format
    fn sign_message(
        &self,
        message: &str,
        path: &DerivationPath,
    ) -> Result<MessageSignature, Self::Error>;
}

#[cfg(feature = "amp0")]
pub mod amp0 {
    use super::*;
    use crate::Network;
    use elements::hex::ToHex;
    use elements::Address;

    /// Signer information necessary for full login to AMP0
    #[allow(unused)]
    pub struct Amp0SignerData {
        // used for register and login_address
        master_xpub: Xpub,
        // used for gait path (not in the client blob)
        register_xpub: Xpub,
        // used for signing the login challenge
        login_xpub: Xpub,
        // used for encrypting the client blob
        client_secret_xpub: Xpub,
        // master blinding key (always slip77)
        slip77_key: MasterBlindingKey,
    }

    impl Amp0SignerData {
        pub fn master_xpub(&self) -> &Xpub {
            &self.master_xpub
        }

        pub fn register_xpub(&self) -> &Xpub {
            &self.register_xpub
        }

        pub fn login_address(&self, network: &Network) -> Address {
            let pk = bitcoin::PublicKey::new(self.master_xpub.public_key);
            let params = network.address_params();
            Address::p2pkh(&pk, None, params)
        }
    }

    /// AMP0 signer methods
    pub trait Amp0Signer: Signer {
        /// AMP0 signer data for login
        fn amp0_signer_data(&self) -> Result<Amp0SignerData, Self::Error> {
            let master_xpub = self.xpub()?;
            let register_path = DerivationPath::from_str("m/18241h").expect("static");
            let register_xpub = self.derive_xpub(&register_path)?;
            // TODO: derive from master xpub
            let login_path = DerivationPath::from_str("m/1195487518").expect("static");
            let login_xpub = self.derive_xpub(&login_path)?;
            let client_secret_path = DerivationPath::from_str("m/1885434739h").expect("static");
            let client_secret_xpub = self.derive_xpub(&client_secret_path)?;

            let slip77_key = self.slip77_master_blinding_key()?;

            Ok(Amp0SignerData {
                master_xpub,
                register_xpub,
                login_xpub,
                client_secret_xpub,
                slip77_key,
            })
        }

        /// AMP0 sign login challenge
        fn amp0_sign_challenge(&self, challenge: &str) -> Result<String, Self::Error> {
            // TODO: validate challenge
            let message = format!("greenaddress.it      login {challenge}");
            let path = DerivationPath::from_str("m/1195487518").expect("static");
            let sig = self.sign_message(&message, &path)?;
            let der_sig = sig.signature.to_standard().serialize_der();
            Ok(der_sig.to_hex())
        }

        /// AMP0 subaccount xpub
        fn amp0_subaccount_xpub(&self, subaccount: u32) -> Result<Xpub, Self::Error> {
            // TODO: return error if subaccount is > 2**31
            let path = DerivationPath::from_str(&format!("m/3h/{}h", subaccount)).expect("TODO");
            self.derive_xpub(&path)
        }
    }
}
