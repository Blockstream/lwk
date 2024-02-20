use wasm_bindgen::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Hex(#[from] lwk_wollet::bitcoin::hashes::hex::HexToArrayError),

    #[error(transparent)]
    Wollet(#[from] lwk_wollet::Error),

    #[error(transparent)]
    Encode(#[from] lwk_wollet::elements::encode::Error),

    #[error(transparent)]
    SerdeJs(#[from] serde_wasm_bindgen::Error),

    #[error(transparent)]
    Address(#[from] lwk_wollet::elements::AddressError),   
}

impl From<Error> for JsValue {
    fn from(val: Error) -> JsValue {
        format!("{val:?}").into()
    }
}
