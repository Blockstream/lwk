use wasm_bindgen::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Hex(#[from] lwk_wollet::bitcoin::hashes::hex::HexToArrayError),

    #[error(transparent)]
    Wollet(#[from] lwk_wollet::Error),
}

impl From<Error> for JsValue {
    fn from(val: Error) -> JsValue {
        format!("{val:?}").into()
    }
}
