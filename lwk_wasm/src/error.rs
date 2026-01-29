use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::js_sys::{self, Reflect};

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

    #[error(transparent)]
    Prices(#[from] lwk_wollet::prices::Error),

    #[error(transparent)]
    Secp256k1(#[from] lwk_wollet::elements::bitcoin::secp256k1::Error),

    #[error(transparent)]
    KeyFromSlice(#[from] lwk_wollet::elements::bitcoin::key::FromSliceError),

    #[error(transparent)]
    FromWif(#[from] lwk_wollet::elements::bitcoin::key::FromWifError),

    #[error(transparent)]
    Secp256k1Zkp(#[from] lwk_wollet::elements::secp256k1_zkp::Error),

    #[error(transparent)]
    Taproot(#[from] lwk_wollet::elements::bitcoin::taproot::TaprootError),

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
                lwk_boltz::Error::LnUrlUnsupported => "Boltz::LnUrlUnsupported",
                lwk_boltz::Error::MnemonicIdentifierMismatch(_, _) => {
                    "Boltz::MnemonicIdentifierMismatch"
                }
                lwk_boltz::Error::NoBoltzUpdate => "Boltz::NoBoltzUpdate",
                lwk_boltz::Error::FailBuildingRefundTransaction => {
                    "Boltz::FailBuildingRefundTransaction"
                }
                lwk_boltz::Error::InvalidSwapPair { .. } => "Boltz::InvalidSwapPair",
                lwk_boltz::Error::MissingQuoteParam(_) => "Boltz::MissingQuoteParam",
                lwk_boltz::Error::PairNotAvailable => "Boltz::PairNotAvailable",
                lwk_boltz::Error::LockPoisoned(_) => "Boltz::LockPoisoned",
                lwk_boltz::Error::Store(_) => "Boltz::Store",
                lwk_boltz::Error::StoreNotConfigured => "Boltz::StoreNotConfigured",
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
            Error::Prices(inner) => match inner {
                lwk_wollet::prices::Error::UnrecognizedCurrency(_) => {
                    "Prices::UnrecognizedCurrency"
                }
                lwk_wollet::prices::Error::UnsupportedCurrency(_) => "Prices::UnsupportedCurrency",
                lwk_wollet::prices::Error::NotEnoughSources(_) => "Prices::NotEnoughSources",
                lwk_wollet::prices::Error::Http(_) => "Prices::Http",
            },
            Error::Secp256k1(_) => "Secp256k1",
            Error::KeyFromSlice(_) => "KeyFromSlice",
            Error::FromWif(_) => "FromWif",
            Error::Secp256k1Zkp(_) => "Secp256k1Zkp",
            Error::Taproot(_) => "Taproot",
            Error::Generic(_) => "Generic",
        }
    }
}

impl From<Error> for JsValue {
    fn from(err: Error) -> Self {
        if let Error::JsVal(e) = err {
            return e;
        }

        let msg = format!("{err}");
        let code = err.code();

        let js_error = js_sys::Error::new(&msg);

        js_error.set_name("LwkError");

        let _ = Reflect::set(&js_error, &JsValue::from("code"), &JsValue::from(code));

        if let Error::Boltz(e) = err {
            if let Ok(magic_routing_hint) = MagicRoutingHint::try_from(e) {
                let _ = Reflect::set(
                    &js_error,
                    &JsValue::from("details"),
                    &magic_routing_hint.into(),
                );
            }
        }

        JsValue::from(js_error)
    }
}

/// A struct representing a magic routing hint, with details on how to pay directly without using Boltz
#[wasm_bindgen]
pub struct MagicRoutingHint {
    address: String,
    amount: u64,
    uri: String,
}

#[wasm_bindgen]
impl MagicRoutingHint {
    /// The address to pay directly to
    pub fn address(&self) -> String {
        self.address.clone()
    }

    /// The amount to pay directly to
    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// The URI to pay directly to
    pub fn uri(&self) -> String {
        self.uri.clone()
    }
}

impl TryFrom<lwk_boltz::Error> for MagicRoutingHint {
    type Error = ();

    fn try_from(err: lwk_boltz::Error) -> Result<Self, Self::Error> {
        match err {
            lwk_boltz::Error::MagicRoutingHint {
                address,
                amount,
                uri,
            } => Ok(Self {
                address,
                amount,
                uri,
            }),
            _ => Err(()),
        }
    }
}
