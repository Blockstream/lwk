use wasm_bindgen::prelude::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Hex(#[from] lwk_wollet::bitcoin::hashes::hex::HexToArrayError),
}

impl Into<JsValue> for Error {
    fn into(self) -> JsValue {
        format!("{self:?}").into()
    }
}
