use wasm_bindgen::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
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

impl From<Error> for JsValue {
    fn from(val: Error) -> JsValue {
        if let Error::JsVal(e) = val {
            e
        } else {
            format!("{val}").into()
        }
    }
}
