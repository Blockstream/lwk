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
use crate::Network;

fn hardened(index: u32) -> Result<ChildNumber, bitcoin::bip32::Error> {
    ChildNumber::from_hardened_idx(index)
}

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

    /// Return the network the signer is configured for
    fn network(&self) -> Result<Network, Self::Error>;

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
        let purpose = match bip {
            Bip::Bip84 => 84,
            Bip::Bip49 => 49,
            Bip::Bip87 => 87,
            Bip::Bip86 => 86,
        };
        let account = 0;
        let path = [purpose, coin_type, account];
        let derivation_path =
            DerivationPath::from_iter(path.map(|index| hardened(index).expect("static")));
        let path = format!("{purpose}h/{coin_type}h/{account}h");

        let fingerprint = self.fingerprint()?;
        let xpub = self.derive_xpub(&derivation_path)?;
        let keyorigin_xpub = format!("[{fingerprint}/{path}]{xpub}");
        Ok(keyorigin_xpub)
    }

    /// Return true if the signer is for mainnet.
    fn is_mainnet(&self) -> Result<bool, Self::Error> {
        Ok(self.network()?.is_mainnet())
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

    /// Derive a "standard" single sig descriptor
    ///
    /// **Experimental**: this API might change without notice.
    ///
    /// These are the "standard" single sig descriptors derived and
    /// used by common Liquid wallets. Their derivation is not
    /// specified in any ELIP.
    ///
    /// The unblinded descriptor follows BIP44/BIP49/BIP84/BIP86.
    ///
    /// They use a SLIP77 descriptor blinding key, however all
    /// accounts use the same descriptor blinding key. This has the
    /// undesirable consequence that if you share the CT descriptor
    /// for one account, you reveal the descriptor blinding key used
    /// by all other accounts.
    fn ss_desc(&self, account_type: SSAccountType, account_num: u32) -> Result<String, Self::Error>
    where
        Self::Error: From<bitcoin::bip32::Error>,
    {
        let network = self.network()?;
        let (prefix, suffix) = account_type.desc_affixes();
        let path = ss_path(&network, account_type, account_num)?;

        let fingerprint = self.fingerprint()?;
        let xpub = self.derive_xpub(&path)?;
        let blinding_key = self.slip77_master_blinding_key()?;

        Ok(format!(
            "ct(slip77({blinding_key}),{prefix}([{fingerprint}/{path}]{xpub}/<0;1>/*){suffix})"
        ))
    }
}

/// Derive the account-level path for a "standard" single sig account.
///
/// **Experimental**: this API might change without notice.
pub fn ss_path(
    network: &Network,
    account_type: SSAccountType,
    account_num: u32,
) -> Result<DerivationPath, bitcoin::bip32::Error> {
    let coin_type = if network.is_mainnet() { 1776 } else { 1 };
    let purpose = account_type.purpose();
    let path = [
        hardened(purpose).expect("static"),
        hardened(coin_type).expect("static"),
        hardened(account_num)?,
    ];
    Ok(DerivationPath::from_iter(path))
}

/// The variant of a "standard" single sig account, see [`Signer::ss_desc`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SSAccountType {
    /// For WPKH accounts (BIP84)
    Wpkh,

    /// For SH-WPKH accounts (BIP49)
    ShWpkh,

    /// For taproot accounts (BIP86)
    Tr,
}

impl SSAccountType {
    fn purpose(self) -> u32 {
        match self {
            SSAccountType::Wpkh => 84,
            SSAccountType::ShWpkh => 49,
            SSAccountType::Tr => 86,
        }
    }

    /// Return the descriptor prefix and suffix
    pub fn desc_affixes(self) -> (&'static str, &'static str) {
        match self {
            SSAccountType::Wpkh => ("elwpkh", ""),
            SSAccountType::ShWpkh => ("elsh(wpkh", ")"),
            SSAccountType::Tr => ("eltr", ""),
        }
    }
}

impl std::fmt::Display for SSAccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SSAccountType::Wpkh => write!(f, "wpkh"),
            SSAccountType::ShWpkh => write!(f, "shwpkh"),
            SSAccountType::Tr => write!(f, "tr"),
        }
    }
}

impl std::str::FromStr for SSAccountType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "wpkh" => Ok(SSAccountType::Wpkh),
            "shwpkh" => Ok(SSAccountType::ShWpkh),
            "tr" => Ok(SSAccountType::Tr),
            _ => Err(
                "Invalid single sig account type, supported variants are: 'wpkh', 'shwpkh', 'tr'",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn ss_account() {
        for el in ["wpkh", "shwpkh", "tr"] {
            let account_type = SSAccountType::from_str(el).unwrap();
            assert_eq!(el, account_type.to_string());
        }
        SSAccountType::from_str("invalid").unwrap_err();

        let mainnet = Network::Liquid;
        let testnet = Network::TestnetLiquid;

        let path = ss_path(&mainnet, SSAccountType::Wpkh, 0).unwrap();
        assert_eq!(path.to_string(), "84'/1776'/0'");
        let path = ss_path(&testnet, SSAccountType::Wpkh, 1).unwrap();
        assert_eq!(path.to_string(), "84'/1'/1'");
        let path = ss_path(&testnet, SSAccountType::ShWpkh, 0).unwrap();
        assert_eq!(path.to_string(), "49'/1'/0'");
        let path = ss_path(&testnet, SSAccountType::Tr, 0).unwrap();
        assert_eq!(path.to_string(), "86'/1'/0'");

        // account_num is caller-supplied and can be out of the hardened range
        assert!(ss_path(&testnet, SSAccountType::Wpkh, 1 << 31).is_err());
    }
}

#[cfg(feature = "amp0")]
pub mod amp0 {
    use super::*;
    use crate::Network;
    use elements::hex::ToHex;
    use elements::Address;
    use serde::{Deserialize, Serialize};
    use serde_json;
    use std::str::FromStr;

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
