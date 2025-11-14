use std::str::FromStr;

use elements::bitcoin::bip32::{DerivationPath, KeySource, Xpub};
use elements::hex::ToHex;
use elements_miniscript::descriptor::checksum::desc_checksum;
use rand::{thread_rng, Rng};
use thiserror::Error;

use crate::Signer;

// TODO impl error handling
/// Generate a singlesig descriptor with the given parameters
pub fn singlesig_desc<S: Signer + ?Sized>(
    signer: &S,
    script_variant: Singlesig,
    blinding_variant: DescriptorBlindingKey,
) -> Result<String, String> {
    let is_mainnet = signer.is_mainnet().map_err(|e| format!("{e:?}"))?;
    let coin_type = if is_mainnet { 1776 } else { 1 };
    let (prefix, path, suffix) = match script_variant {
        Singlesig::Wpkh => ("elwpkh", format!("84h/{coin_type}h/0h"), ""),
        Singlesig::ShWpkh => ("elsh(wpkh", format!("49h/{coin_type}h/0h"), ")"),
    };

    let fingerprint = signer.fingerprint().map_err(|e| format!("{e:?}"))?;

    let xpub = signer
        .derive_xpub(
            &DerivationPath::from_str(&format!("m/{path}")).map_err(|e| format!("{e:?}"))?,
        )
        .map_err(|e| format!("{e:?}"))?;

    let blinding_key = match blinding_variant {
        DescriptorBlindingKey::Slip77 => format!(
            "slip77({})",
            signer
                .slip77_master_blinding_key()
                .map_err(|e| format!("{e:?}"))?
        ),
        DescriptorBlindingKey::Slip77Rand => {
            return Err("Random slip77 key not supported in singlesig descriptor generation".into())
        }
        DescriptorBlindingKey::Elip151 => "elip151".to_string(),
    };

    // m / purpose' / coin_type' / account' / change / address_index
    let desc = format!("ct({blinding_key},{prefix}([{fingerprint}/{path}]{xpub}/<0;1>/*){suffix})");
    let checksum = desc_checksum(&desc).map_err(|e| format!("{e:?}"))?;
    Ok(format!("{desc}#{checksum}"))
}

fn fmt_path(path: &DerivationPath) -> String {
    path.to_string().replace("m/", "").replace('\'', "h")
}

// TODO impl error handling
/// Generate a multisig descriptor with the given parameters
pub fn multisig_desc(
    threshold: u32,
    xpubs: Vec<(Option<KeySource>, Xpub)>,
    script_variant: Multisig,
    blinding_variant: DescriptorBlindingKey,
) -> Result<String, String> {
    if threshold == 0 {
        return Err("Threshold cannot be 0".into());
    } else if threshold as usize > xpubs.len() {
        return Err("Threshold cannot be greater than the number of xpubs".into());
    }

    let (prefix, suffix) = match script_variant {
        Multisig::Wsh => ("elwsh(multi", ")"),
    };

    let blinding_key = match blinding_variant {
        DescriptorBlindingKey::Slip77 => {
            return Err(
                "Deterministic slip77 key not supported in multisig descriptor generation".into(),
            )
        }
        DescriptorBlindingKey::Slip77Rand => {
            let mut bytes = [0u8; 32];
            thread_rng().fill(&mut bytes);
            format!("slip77({})", bytes.to_hex())
        }
        DescriptorBlindingKey::Elip151 => "elip151".to_string(),
    };

    let xpubs = xpubs
        .iter()
        .map(|(keyorigin, xpub)| {
            let prefix = if let Some((fingerprint, path)) = keyorigin {
                format!("[{fingerprint}/{}]", fmt_path(path))
            } else {
                "".to_string()
            };
            format!("{prefix}{xpub}/<0;1>/*")
        })
        .collect::<Vec<_>>()
        .join(",");
    let desc = format!("ct({blinding_key},{prefix}({threshold},{xpubs}){suffix})");
    let checksum = desc_checksum(&desc).map_err(|e| format!("{e:?}"))?;
    Ok(format!("{desc}#{checksum}"))
}

#[derive(Debug, Clone, Copy)]
/// The variant of the singlesig descriptor
pub enum Singlesig {
    /// Witness public key hash as defined by bip84
    Wpkh,

    /// Witness public key hash wrapped in script hash as defined by bip49
    ShWpkh,
}

/// The error type returned by Singlesig::from_str
#[derive(Error, Debug)]
#[error("Invalid singlesig variant '{0}' supported variant are: 'wpkh', 'shwpkh'")]
pub struct InvalidSinglesigVariant(String);

impl FromStr for Singlesig {
    type Err = InvalidSinglesigVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "wpkh" => Singlesig::Wpkh,
            "shwpkh" => Singlesig::ShWpkh,
            v => return Err(InvalidSinglesigVariant(v.to_string())),
        })
    }
}

/// Some descriptor blinding keys variant
///
/// Not all the possible cases described in
/// [ELIP150](<https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki>)
/// have a corresponding variant in this enum.
#[derive(Debug, Clone, Copy)]
pub enum DescriptorBlindingKey {
    /// Deterministic [SLIP77](<https://github.com/satoshilabs/slips/blob/master/slip-0077.md>) master blinding key
    ///
    /// Derived from the BIP32 seed.
    Slip77,

    /// Random [SLIP77](<https://github.com/satoshilabs/slips/blob/master/slip-0077.md>) master blinding key
    ///
    /// Randomly generated SLIP77 master blinding key.
    /// Useful fot cases where the seed isn't available or is not well defined (e.g. multisig).
    ///
    /// Note that single blinding keys are derived _deterministically_ from this SLIP77 master
    /// blinding key.
    Slip77Rand,

    /// [ELIP151](<https://github.com/ElementsProject/ELIPs/blob/main/elip-0151.mediawiki>) descriptor blinding key
    ///
    /// Derived from the ordinary descriptor.
    Elip151,
}

/// The error type returned by `DescriptorBlindingKey::from_str`
#[derive(Error, Debug)]
#[error("Invalid blinding key variant '{0}' supported variant are: 'slip77', 'elip151'")]
pub struct InvalidBlindingKeyVariant(String);

impl FromStr for DescriptorBlindingKey {
    type Err = InvalidBlindingKeyVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "slip77" => DescriptorBlindingKey::Slip77,
            "slip77-rand" => DescriptorBlindingKey::Slip77Rand,
            "elip151" => DescriptorBlindingKey::Elip151,
            v => return Err(InvalidBlindingKeyVariant(v.to_string())),
        })
    }
}

/// The variant of the descriptor like specified in the bips
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Bip {
    /// For P2WPKH wallets
    Bip84,
    /// For P2SH-P2WPKH wallets
    Bip49,
    /// For multisig wallets
    Bip87,
}

impl std::fmt::Display for Bip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bip::Bip84 => write!(f, "bip84"),
            Bip::Bip49 => write!(f, "bip49"),
            Bip::Bip87 => write!(f, "bip87"),
        }
    }
}

/// The error type returned by Bip::from_str
#[derive(Error, Debug)]
#[error("Invalid bip  variant '{0}' supported variant are: 'bip84'")]
pub struct InvalidBipVariant(String);

impl FromStr for Bip {
    type Err = InvalidBipVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "bip84" => Bip::Bip84,
            "bip49" => Bip::Bip49,
            "bip87" => Bip::Bip87,
            v => return Err(InvalidBipVariant(v.to_string())),
        })
    }
}

/// The variant of the multisig descriptor
pub enum Multisig {
    /// Witness script hash
    Wsh,
}

/// The variant of the multisig descriptor
#[derive(Error, Debug)]
#[error("Invalid multisig variant '{0}' supported variant are: 'wsh'")]
pub struct InvalidMultisigVariant(String);

impl FromStr for Multisig {
    type Err = InvalidMultisigVariant;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "wsh" => Multisig::Wsh,
            v => return Err(InvalidMultisigVariant(v.to_string())),
        })
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::Bip;

    #[test]
    fn roundtrip_bip() {
        for el in ["bip49", "bip84", "bip87"] {
            let bip = Bip::from_str(el).unwrap();
            let bip_str = bip.to_string();
            assert_eq!(el, bip_str);
        }
        Bip::from_str("vattelapesca").unwrap_err();
    }
}
