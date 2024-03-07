use crate::{
    serial::{get_jade_serial, WebSerial},
    Error, Network,
};
use lwk_jade::asyncr;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Jade {
    inner: asyncr::Jade<WebSerial>,
    _port: web_sys::SerialPort,
}

#[wasm_bindgen]
impl Jade {
    /// Creates a Jade from Web Serial
    #[wasm_bindgen(constructor)]
    pub async fn from_serial(network: Network) -> Result<Jade, Error> {
        let port = get_jade_serial(false).await?;
        let web_serial = WebSerial::new(&port)?;

        let inner = asyncr::Jade::new(web_serial, network.into());
        Ok(Jade { inner, _port: port })
    }

    pub async fn get_version(&self) -> Result<JsValue, Error> {
        let version = self.inner.version_info().await?;
        Ok(serde_wasm_bindgen::to_value(&version)?)
    }
}
