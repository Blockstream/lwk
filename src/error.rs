use crate::store::StoreMeta;
use aes_gcm_siv::aead;
use bip39;
use serde::ser::Serialize;
use std::convert::From;
use std::fmt::Display;
use std::string::ToString;
use std::sync::{PoisonError, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug)]
pub enum Error {
    Generic(String),
    InvalidMnemonic(bip39::Error),
    AddrParse(String),
    Bitcoin(elements::bitcoin::Error),
    BitcoinHashes(elements::bitcoin::hashes::error::Error),
    BitcoinBIP32Error(elements::bitcoin::bip32::Error),
    BitcoinConsensus(elements::bitcoin::consensus::encode::Error),
    JSON(serde_json::error::Error),
    JsonFrom(serde_json::Error),
    StdIOError(std::io::Error),
    Hex(hex::FromHexError),
    ClientError(electrum_client::Error),
    SliceConversionError(std::array::TryFromSliceError),
    ElementsEncode(elements::encode::Error),
    ElementsPset(elements::pset::Error),
    PsetBlindError(elements::pset::PsetBlindError),
    Send(std::sync::mpsc::SendError<()>),
    Secp256k1(elements::bitcoin::secp256k1::Error),
    Secp256k1Zkp(elements::secp256k1_zkp::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Error::Generic(ref strerr) => write!(f, "{}", strerr),
            Error::InvalidMnemonic(ref mnemonic_err) => {
                write!(f, "invalid mnemonic: {}", mnemonic_err)
            }
            Error::AddrParse(ref addr) => write!(f, "could not parse SocketAddr `{}`", addr),
            Error::Bitcoin(ref btcerr) => write!(f, "bitcoin: {}", btcerr),
            Error::BitcoinHashes(ref btcerr) => write!(f, "bitcoin_hashes: {}", btcerr),
            Error::BitcoinBIP32Error(ref bip32err) => write!(f, "bip32: {}", bip32err),
            Error::BitcoinConsensus(ref consensus_err) => write!(f, "consensus: {}", consensus_err),
            Error::JSON(ref json_err) => write!(f, "json: {}", json_err),
            Error::JsonFrom(ref json_from_err) => write!(f, "json from: {}", json_from_err),
            Error::StdIOError(ref io_err) => write!(f, "io: {}", io_err),
            Error::Hex(ref hex_err) => write!(f, "hex: {}", hex_err),
            Error::ClientError(ref client_err) => write!(f, "client: {:?}", client_err),
            Error::SliceConversionError(ref slice_err) => write!(f, "slice: {}", slice_err),
            Error::ElementsEncode(ref el_err) => write!(f, "el_err: {}", el_err),
            Error::ElementsPset(ref err) => write!(f, "el_pset_err: {}", err),
            Error::PsetBlindError(ref err) => write!(f, "pset_blind_err: {}", err),
            Error::Send(ref send_err) => write!(f, "send_err: {:?}", send_err),
            Error::Secp256k1(ref err) => write!(f, "Secp256k1_err: {:?}", err),
            Error::Secp256k1Zkp(ref err) => write!(f, "Secp256k1_zkp_err: {:?}", err),
        }
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::JsonFrom(e)
    }
}

macro_rules! impl_error {
    ( $from:ty ) => {
        impl std::convert::From<$from> for Error {
            fn from(err: $from) -> Self {
                Error::Generic(err.to_string())
            }
        }
    };
}

impl_error!(&str);
impl_error!(elements::bitcoin::base58::Error);
impl_error!(elements::address::AddressError);
impl_error!(elements::bitcoin::address::Error);
impl_error!(aead::Error);
impl_error!(PoisonError<RwLockReadGuard<'_, StoreMeta>>);
impl_error!(PoisonError<RwLockWriteGuard<'_, StoreMeta>>);
impl_error!(serde_cbor::error::Error);
impl_error!(elements::bitcoin::hashes::hex::Error);
impl_error!(std::string::FromUtf8Error);
impl_error!(elements::bitcoin::key::Error);

impl From<std::array::TryFromSliceError> for Error {
    fn from(err: std::array::TryFromSliceError) -> Self {
        Error::SliceConversionError(err)
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(_err: std::net::AddrParseError) -> Self {
        Error::AddrParse("SocketAddr parse failure with no additional info".into())
    }
}

impl From<elements::bitcoin::bip32::Error> for Error {
    fn from(err: elements::bitcoin::bip32::Error) -> Self {
        Error::BitcoinBIP32Error(err)
    }
}

impl From<String> for Error {
    fn from(err: String) -> Self {
        Error::Generic(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::StdIOError(err)
    }
}

impl From<elements::bitcoin::consensus::encode::Error> for Error {
    fn from(err: elements::bitcoin::consensus::encode::Error) -> Self {
        Error::BitcoinConsensus(err)
    }
}

impl From<hex::FromHexError> for Error {
    fn from(err: hex::FromHexError) -> Self {
        Error::Hex(err)
    }
}

impl From<electrum_client::Error> for Error {
    fn from(err: electrum_client::Error) -> Self {
        Error::ClientError(err)
    }
}

impl From<elements::bitcoin::hashes::error::Error> for Error {
    fn from(err: elements::bitcoin::hashes::error::Error) -> Self {
        Error::BitcoinHashes(err)
    }
}
impl From<elements::encode::Error> for Error {
    fn from(err: elements::encode::Error) -> Self {
        Error::ElementsEncode(err)
    }
}

impl From<elements::pset::Error> for Error {
    fn from(err: elements::pset::Error) -> Self {
        Error::ElementsPset(err)
    }
}

impl From<elements::pset::PsetBlindError> for Error {
    fn from(err: elements::pset::PsetBlindError) -> Self {
        Error::PsetBlindError(err)
    }
}

impl From<std::sync::mpsc::SendError<()>> for Error {
    fn from(err: std::sync::mpsc::SendError<()>) -> Self {
        Error::Send(err)
    }
}

impl From<elements::bitcoin::secp256k1::Error> for Error {
    fn from(err: elements::bitcoin::secp256k1::Error) -> Self {
        Error::Secp256k1(err)
    }
}

impl From<elements::secp256k1_zkp::Error> for Error {
    fn from(err: elements::secp256k1_zkp::Error) -> Self {
        Error::Secp256k1Zkp(err)
    }
}

impl From<bip39::Error> for Error {
    fn from(err: bip39::Error) -> Self {
        Error::InvalidMnemonic(err)
    }
}
