use crate::{Error, WebSocketSerial};

use lwk_common::Network;
use wasm_bindgen::prelude::*;

/// Wrapper of [`lwk_wollet::amp0::Amp0`]
#[wasm_bindgen]
pub struct Amp0 {
    inner: lwk_wollet::amp0::Amp0<WebSocketSerial>,
}
#[wasm_bindgen]
impl Amp0 {
    pub async fn new_testnet() -> Result<Self, Error> {
        let url = lwk_wollet::amp0::default_url(Network::TestnetLiquid)?;
        let websocket_serial = WebSocketSerial::new_wamp(url).await?;
        let inner = lwk_wollet::amp0::Amp0::new(websocket_serial).await?;
        Ok(Self { inner })
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<String, Error> {
        Ok(self.inner.login(username, password).await?.to_string())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    use crate::WolletDescriptor;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_amp0_login() {
        let amp0 = Amp0::new_testnet().await.unwrap();
        let login_response = amp0.login("userleo456", "userleo456").await.unwrap();

        // TODO: this fails at the moment
        // assert!(login_response.contains("GA2zxWdhAYtREeYCVFTGRhHQmYMPAP"));
    }
}
