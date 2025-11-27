use wasm_bindgen::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Boltz(#[from] lwk_boltz::Error),

    #[error(transparent)]
    HexToArray(#[from] lwk_wollet::bitcoin::hashes::hex::HexToArrayError),

    #[error(transparent)]
    Wollet(#[from] lwk_wollet::Error),

    #[error(transparent)]
    Encode(#[from] lwk_wollet::elements::encode::Error),

    #[error(transparent)]
    SerdeJs(#[from] serde_wasm_bindgen::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Address(#[from] lwk_wollet::elements::AddressError),

    #[error(transparent)]
    HexToBytes(#[from] lwk_wollet::bitcoin::hashes::hex::HexToBytesError),

    #[error(transparent)]
    Pset(#[from] lwk_wollet::elements::pset::Error),

    #[error(transparent)]
    PsetParse(#[from] lwk_wollet::elements::pset::ParseError),

    #[error(transparent)]
    ParseOutPoint(#[from] lwk_wollet::elements::bitcoin::transaction::ParseOutPointError),

    #[error(transparent)]
    Bip39(#[from] lwk_signer::bip39::Error),

    #[error(transparent)]
    Bip32(#[from] lwk_wollet::bitcoin::bip32::Error),

    #[error(transparent)]
    Sign(#[from] lwk_signer::SignError),

    #[error(transparent)]
    SignerNew(#[from] lwk_signer::NewError),

    #[error(transparent)]
    Jade(#[from] lwk_jade::Error),

    #[error(transparent)]
    Qr(#[from] lwk_common::QrError),

    #[error(transparent)]
    Keyorigin(#[from] lwk_common::InvalidKeyOriginXpub),

    #[error(transparent)]
    Precision(#[from] lwk_common::precision::Error),

    #[error(transparent)]
    AddressParse(#[from] lwk_common::AddressParseError),

    #[error("{0}")]
    Generic(String),

    #[error("{0:?}")]
    JsVal(JsValue),
}

impl Error {
    /// Returns the error code as a string for JS discrimination
    fn code(&self) -> &'static str {
        match self {
            Error::JsVal(_) => "JsVal",
            Error::Boltz(inner) => match inner {
                lwk_boltz::Error::MagicRoutingHint { .. } => "Boltz::MagicRoutingHint",
                lwk_boltz::Error::InvalidElectrumUrl(_) => "Boltz::InvalidElectrumUrl",
                lwk_boltz::Error::InvalidSwapState(_) => "Boltz::InvalidSwapState",
                lwk_boltz::Error::InvalidBolt11Invoice(_) => "Boltz::InvalidBolt11Invoice",
                lwk_boltz::Error::BoltzApi(_) => "Boltz::BoltzApi",
                lwk_boltz::Error::ElementsAddressError(_) => "Boltz::ElementsAddressError",
                lwk_boltz::Error::Receiver(_) => "Boltz::Receiver",
                lwk_boltz::Error::TryReceiver(_) => "Boltz::TryReceiver",
                lwk_boltz::Error::UnexpectedUpdate { .. } => "Boltz::UnexpectedUpdate",
                lwk_boltz::Error::InvoiceWithoutAmount(_) => "Boltz::InvoiceWithoutAmount",
                lwk_boltz::Error::ExpectedAmountLowerThanInvoice(_, _) => {
                    "Boltz::ExpectedAmountLowerThanInvoice"
                }
                lwk_boltz::Error::MissingInvoiceInResponse(_) => "Boltz::MissingInvoiceInResponse",
                lwk_boltz::Error::InvoiceWithoutMagicRoutingHint(_) => {
                    "Boltz::InvoiceWithoutMagicRoutingHint"
                }
                lwk_boltz::Error::Timeout(_) => "Boltz::Timeout",
                lwk_boltz::Error::Io(_) => "Boltz::Io",
                lwk_boltz::Error::SerdeJson(_) => "Boltz::SerdeJson",
                lwk_boltz::Error::Bip32(_) => "Boltz::Bip32",
                lwk_boltz::Error::Secp256k1(_) => "Boltz::Secp256k1",
                lwk_boltz::Error::Expired { .. } => "Boltz::Expired",
                lwk_boltz::Error::SwapRestoration(_) => "Boltz::SwapRestoration",
                lwk_boltz::Error::RetryBroadcastFailed => "Boltz::RetryBroadcastFailed",
                lwk_boltz::Error::Bolt12Unsupported => "Boltz::Bolt12Unsupported",
                lwk_boltz::Error::MnemonicIdentifierMismatch(_, _) => {
                    "Boltz::MnemonicIdentifierMismatch"
                }
                lwk_boltz::Error::NoBoltzUpdate => "Boltz::NoBoltzUpdate",
            },
            Error::HexToArray(_) => "HexToArray",
            Error::Wollet(_) => "Wollet",
            Error::Encode(_) => "Encode",
            Error::SerdeJs(_) => "SerdeJs",
            Error::SerdeJson(_) => "SerdeJson",
            Error::Address(_) => "Address",
            Error::HexToBytes(_) => "HexToBytes",
            Error::Pset(_) => "Pset",
            Error::PsetParse(_) => "PsetParse",
            Error::ParseOutPoint(_) => "ParseOutPoint",
            Error::Bip39(_) => "Bip39",
            Error::Bip32(_) => "Bip32",
            Error::Sign(_) => "Sign",
            Error::SignerNew(_) => "SignerNew",
            Error::Jade(_) => "Jade",
            Error::Qr(_) => "Qr",
            Error::Keyorigin(_) => "Keyorigin",
            Error::Precision(_) => "Precision",
            Error::AddressParse(_) => "AddressParse",
            Error::Generic(_) => "Generic",
        }
    }
}

impl From<Error> for JsValue {
    fn from(val: Error) -> JsValue {
        if let Error::JsVal(e) = val {
            e
        } else {
            format!("{val}").into()
        }
    }
}
