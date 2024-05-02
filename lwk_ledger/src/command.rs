/// APDU commands  for the Bitcoin application.
///
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

/// Creates the APDU command to CONTINUE.
pub fn continue_interrupted(data: Vec<u8>) -> APDUCommand {
    APDUCommand {
        cla: apdu::Cla::Framework as u8,
        ins: apdu::FrameworkCommandCode::ContinueInterrupted as u8,
        data,
        ..Default::default()
    }
}
