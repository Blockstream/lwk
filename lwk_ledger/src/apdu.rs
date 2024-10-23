use core::convert::TryFrom;
use core::fmt::Debug;

use ledger_apdu::APDUCommand;

// p2 encodes the protocol version implemented
pub const CURRENT_PROTOCOL_VERSION: u8 = 1;

pub type APDUCmdVec = APDUCommand<Vec<u8>>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cla {
    Default = 0xB0,
    Bitcoin = 0xE1,
    Framework = 0xF8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LiquidCommandCode {
    GetExtendedPubkey = 0x00,

    /// This variant is used for two commands depending on the class:
    ///
    /// * With `Cla::Default` is GetVersion
    /// * With `Cla::Framework` is ContinueInterrupted
    GetVersionOrContinueInterrupted = 0x01,

    RegisterWallet = 0x02,
    GetWalletAddress = 0x03,
    SignPSBT = 0x04,
    GetMasterFingerprint = 0x05,
    SignMessage = 0x10,
    // Liquid commands
    LiquidGetMasterBlindingKey = 0xe1,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ClientCommandCode {
    Yield = 0x10,
    GetPreimage = 0x40,
    GetMerkleLeafProof = 0x41,
    GetMerkleLeafIndex = 0x42,
    GetMoreElements = 0xA0,
}

impl TryFrom<u8> for ClientCommandCode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x10 => Ok(ClientCommandCode::Yield),
            0x40 => Ok(ClientCommandCode::GetPreimage),
            0x41 => Ok(ClientCommandCode::GetMerkleLeafProof),
            0x42 => Ok(ClientCommandCode::GetMerkleLeafIndex),
            0xA0 => Ok(ClientCommandCode::GetMoreElements),
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum StatusWord {
    /// Rejected by user
    Deny = 0x6985,
    /// Incorrect Data
    IncorrectData = 0x6A80,
    /// Not Supported
    NotSupported = 0x6A82,
    /// Wrong P1P2
    WrongP1P2 = 0x6A86,
    /// Wrong DataLength
    WrongDataLength = 0x6A87,
    /// Ins not supported
    InsNotSupported = 0x6D00,
    /// Cla not supported
    ClaNotSupported = 0x6E00,
    /// Bad state
    BadState = 0xB007,
    /// Signature fail
    SignatureFail = 0xB008,
    /// Success
    OK = 0x9000,
    /// The command is interrupted, and requires the client's response
    InterruptedExecution = 0xE000,
    /// Unknown
    Unknown,
}

impl TryFrom<u16> for StatusWord {
    type Error = ();

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x6985 => Ok(StatusWord::Deny),
            0x6A80 => Ok(StatusWord::IncorrectData),
            0x6A82 => Ok(StatusWord::NotSupported),
            0x6A86 => Ok(StatusWord::WrongP1P2),
            0x6A87 => Ok(StatusWord::WrongDataLength),
            0x6D00 => Ok(StatusWord::InsNotSupported),
            0x6E00 => Ok(StatusWord::ClaNotSupported),
            0xB007 => Ok(StatusWord::BadState),
            0xB008 => Ok(StatusWord::SignatureFail),
            0x9000 => Ok(StatusWord::OK),
            0xE000 => Ok(StatusWord::InterruptedExecution),
            _ => Err(()),
        }
    }
}

pub fn apdu(cla: Cla, ins: LiquidCommandCode, data: Vec<u8>) -> APDUCmdVec {
    APDUCmdVec {
        cla: cla as u8,
        ins: ins as u8,
        p1: 0x00,
        p2: CURRENT_PROTOCOL_VERSION,
        data,
    }
}
pub fn apdu_empty(cla: Cla, ins: LiquidCommandCode) -> APDUCmdVec {
    apdu(cla, ins, Vec::new())
}
