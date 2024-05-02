/// APDU commands  for the Bitcoin application.
///
use bitcoin::{
    bip32::{ChildNumber, DerivationPath},
    consensus::encode::{self, VarInt},
};
use core::default::Default;

use super::apdu::{self, APDUCommand};

/// Creates the APDU Command to retrieve the app's name, version and state flags.
pub fn get_version() -> APDUCommand {
    APDUCommand {
        ins: apdu::LiquidCommandCode::GetVersion as u8,
        p2: 0x00,
        ..Default::default()
    }
}

/// Creates the APDU Command to retrieve the master fingerprint.
pub fn get_master_fingerprint() -> APDUCommand {
    APDUCommand {
        cla: apdu::Cla::Bitcoin as u8,
        ins: apdu::LiquidCommandCode::GetMasterFingerprint as u8,
        ..Default::default()
    }
}

/// Creates the APDU command required to get the extended pubkey with the given derivation path.
pub fn get_extended_pubkey(path: &DerivationPath, display: bool) -> APDUCommand {
    let child_numbers: &[ChildNumber] = path.as_ref();
    let data: Vec<u8> = child_numbers.iter().fold(
        vec![
            if display { 1_u8 } else { b'\0' },
            child_numbers.len() as u8,
        ],
        |mut acc, &x| {
            acc.extend_from_slice(&u32::from(x).to_be_bytes());
            acc
        },
    );

    APDUCommand {
        cla: apdu::Cla::Bitcoin as u8,
        ins: apdu::LiquidCommandCode::GetExtendedPubkey as u8,
        data,
        ..Default::default()
    }
}

/// Creates the APDU Command to sign a message.
pub fn sign_message(
    message_length: usize,
    message_commitment_root: &[u8; 32],
    path: &DerivationPath,
) -> APDUCommand {
    let child_numbers: &[ChildNumber] = path.as_ref();
    let mut data: Vec<u8> =
        child_numbers
            .iter()
            .fold(vec![child_numbers.len() as u8], |mut acc, &x| {
                acc.extend_from_slice(&u32::from(x).to_be_bytes());
                acc
            });
    data.extend(encode::serialize(&VarInt(message_length as u64)));
    data.extend_from_slice(message_commitment_root);

    APDUCommand {
        cla: apdu::Cla::Bitcoin as u8,
        ins: apdu::LiquidCommandCode::SignMessage as u8,
        data,
        ..Default::default()
    }
}

/// Creates the APDU command to CONTINUE.
pub fn continue_interrupted(data: Vec<u8>) -> APDUCommand {
    APDUCommand {
        cla: apdu::Cla::Framework as u8,
        ins: apdu::FrameworkCommandCode::ContinueInterrupted as u8,
        data,
        ..Default::default()
    }
}
