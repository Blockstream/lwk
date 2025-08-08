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

#[wasm_bindgen]
pub fn encrypt_credentials(username: &str, password: &str) -> String {
    let (u, p) = lwk_wollet::amp0::encrypt_credentials(username, password);
    format!("{}:{}", u, p)
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    use crate::WolletDescriptor;
    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_amp0_just_connect() {
        let _amp0 = Amp0::new_testnet().await.unwrap();
    }

    #[wasm_bindgen_test]
    async fn test_amp0_login() {
        let amp0 = Amp0::new_testnet().await.unwrap();
        let login_response = amp0.login("userleo456", "userleo456").await.unwrap();

        // TODO: this fails at the moment
        // assert!(login_response.contains("GA2zxWdhAYtREeYCVFTGRhHQmYMPAP"));

        assert_eq!(login_response, ""); // just to see return value
    }

    #[wasm_bindgen_test]
    async fn test_encrypt_credentials() {
        let encrypted = super::encrypt_credentials("userleo456", "userleo456");
        assert_eq!(encrypted, "a3c7f7de9a34bcab4554f7cedf6046e041eeb3a9211466d92ecaa9763ac3557b:f3ac0f33fe97412a39ebb5d11d111961a754ecbbbdf12c71342adb7022ae3a2d");
    }
}
