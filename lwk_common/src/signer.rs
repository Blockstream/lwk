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
    /// The user defined error type returned by the signer.
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

    /// Return true if the signer is for mainnet.
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

    /// Return the Witness Public Key Hash, slip77, descriptor for this signer
    ///
    /// Example: "ct(slip77(...),elwpkh([73c5da0a/84'/1'/0']xpub.../<0;1>/*))#2e4n992d"
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
    use serde::{Deserialize, Serialize};
    use serde_json;

    /// Signer information necessary for full login to AMP0
    ///
    /// Consists in a series of xpubs and the SLIP77 master
    /// blinding key. These data must be obtained from a signer
    /// for logging in AMP0.
    ///
    /// In general the signer is isolated, so we need to be able
    /// (de)serialize this struct.
    #[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
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
        slip77_key: String,
    }

    impl std::fmt::Display for Amp0SignerData {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match serde_json::to_string(self) {
                Ok(s) => write!(f, "{s}"),
                Err(e) => write!(f, "Error serializing: {e}"),
            }
        }
    }

    impl FromStr for Amp0SignerData {
        type Err = serde_json::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            serde_json::from_str(s)
        }
    }

    impl Amp0SignerData {
        /// Return the master xpub used for register and login_address
        pub fn master_xpub(&self) -> &Xpub {
            &self.master_xpub
        }

        /// Return the register xpub used for gait path (not in the client blob)
        pub fn register_xpub(&self) -> &Xpub {
            &self.register_xpub
        }

        /// Return the login xpub used for signing the login challenge
        pub fn login_xpub(&self) -> &Xpub {
            &self.login_xpub
        }

        /// Return the client secret xpub used for encrypting the client blob
        pub fn client_secret_xpub(&self) -> &Xpub {
            &self.client_secret_xpub
        }

        /// Return the login address (used for the login challenge)
        pub fn login_address(&self, network: &Network) -> Address {
            let pk = bitcoin::PublicKey::new(self.master_xpub.public_key);
            let params = network.address_params();
            Address::p2pkh(&pk, None, params)
        }

        /// Return the slip77 master blinding key
        pub fn slip77_key(&self) -> &str {
            &self.slip77_key
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

            let slip77_key = self.slip77_master_blinding_key()?.to_string();

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

        /// AMP0 account xpub
        fn amp0_account_xpub(&self, account: u32) -> Result<Xpub, Self::Error> {
            // TODO: return error if account is > 2**31
            let path = DerivationPath::from_str(&format!("m/3h/{account}h")).expect("TODO");
            self.derive_xpub(&path)
        }
    }
}
