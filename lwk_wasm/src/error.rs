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
    Sign(#[from] lwk_signer::SignError),

    #[error(transparent)]
    SignerNew(#[from] lwk_signer::NewError),

    #[error("{0}")]
    Generic(String),
}

impl From<Error> for JsValue {
    fn from(val: Error) -> JsValue {
        format!("{val:?}").into()
    }
}
