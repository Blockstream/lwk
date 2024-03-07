use crate::{
    serial::{get_jade_serial, WebSerial},
    Error, Network,
};
use lwk_jade::asyncr;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
struct Jade {
    inner: asyncr::Jade<WebSerial>,
    port: web_sys::SerialPort,
}

#[wasm_bindgen]
impl Jade {
    /// Creates a Jade from Web Serial
    #[wasm_bindgen(constructor)]
    pub async fn from_serial(network: Network) -> Result<Jade, Error> {
        let port = get_jade_serial(false).await?;
        let web_serial = WebSerial::new(&port)?;

        let inner = asyncr::Jade::new(web_serial, network.into());
        Ok(Jade { inner, port })
    }
}
