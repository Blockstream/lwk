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
    /// Creates a Jade from Web Serial for the given network
    ///
    /// When filter is true, it will filter available serial with Blockstream released chips, use
    /// false if you don't see your DYI jade
    #[wasm_bindgen(constructor)]
    pub async fn from_serial(network: Network, filter: bool) -> Result<Jade, Error> {
        let port = get_jade_serial(filter).await?;
        let web_serial = WebSerial::new(&port)?;

        let inner = asyncr::Jade::new(web_serial, network.into());
        Ok(Jade { inner, _port: port })
    }

    #[wasm_bindgen(js_name = getVersion)]
    pub async fn get_version(&self) -> Result<JsValue, Error> {
        let version = self.inner.version_info().await?;
        Ok(serde_wasm_bindgen::to_value(&version)?)
    }

    #[wasm_bindgen(js_name = getMasterXpub)]
    pub async fn get_master_xpub(&self) -> Result<String, Error> {
        self.inner.unlock().await?;
        let xpub = self.inner.get_master_xpub().await?;
        Ok(xpub.to_string())
    }
}
