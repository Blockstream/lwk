use crate::{AddressResult, Error, Network, WebSocketSerial};

use wasm_bindgen::prelude::*;

/// Wrapper of [`lwk_wollet::amp0::Amp0`]
#[wasm_bindgen]
pub struct Amp0 {
    inner: lwk_wollet::amp0::Amp0<WebSocketSerial>,
}
#[wasm_bindgen]
impl Amp0 {
    pub async fn new_with_network(network: Network) -> Result<Self, Error> {
        let url = lwk_wollet::amp0::default_url(network.into())?;
        let websocket_serial = WebSocketSerial::new_wamp(url).await?;
        let inner = lwk_wollet::amp0::Amp0::new(websocket_serial).await?;
        Ok(Self { inner })
    }

    pub async fn new_testnet() -> Result<Self, Error> {
        Self::new_with_network(Network::testnet()).await
    }

    pub async fn new_mainnet() -> Result<Self, Error> {
        Self::new_with_network(Network::mainnet()).await
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<String, Error> {
        let login_data = self.inner.login(username, password).await?;
        Ok(serde_json::to_string(&login_data)?)
    }
}

#[wasm_bindgen]
pub fn encrypt_credentials(username: &str, password: &str) -> String {
    let (u, p) = lwk_wollet::amp0::encrypt_credentials(username, password);
    format!("{}:{}", u, p)
}

/// Wrapper of [`lwk_wollet::amp0::Amp0Ext`]
#[wasm_bindgen]
pub struct Amp0Ext {
    inner: lwk_wollet::amp0::Amp0Ext<WebSocketSerial>,
    network: Network,
}

#[wasm_bindgen]
impl Amp0Ext {
    async fn new_with_network(
        network: Network,
        username: &str,
        password: &str,
        amp_id: &str,
    ) -> Result<Self, Error> {
        let url = lwk_wollet::amp0::default_url(network.into())?;
        let websocket_serial = WebSocketSerial::new_wamp(url).await?;
        let amp0 = lwk_wollet::amp0::Amp0::new(websocket_serial).await?;
        let inner =
            lwk_wollet::amp0::Amp0Ext::new(amp0, network.into(), username, password, amp_id)
                .await?;
        Ok(Self { inner, network })
    }

    pub async fn new_testnet(username: &str, password: &str, amp_id: &str) -> Result<Self, Error> {
        Self::new_with_network(Network::testnet(), username, password, amp_id).await
    }

    pub async fn new_mainnet(username: &str, password: &str, amp_id: &str) -> Result<Self, Error> {
        Self::new_with_network(Network::mainnet(), username, password, amp_id).await
    }

    pub fn last_index(&self) -> u32 {
        self.inner.last_index
    }

    pub async fn address(&mut self, index: Option<u32>) -> Result<AddressResult, Error> {
        let address_result = self.inner.address(index).await?;
        Ok(address_result.into())
    }

    pub fn wollet(&self) -> Result<crate::Wollet, Error> {
        Ok(crate::Wollet::new(
            &self.network,
            &self.inner.wollet.wollet_descriptor().into(),
        )?)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_amp0_just_connect() {
        let _amp0 = Amp0::new_testnet().await.unwrap();
    }

    #[wasm_bindgen_test]
    async fn test_amp0_login() {
        let amp0 = Amp0::new_mainnet().await.unwrap();
        let login_response = amp0.login("userleo456", "userleo456").await.unwrap();

        assert!(login_response.contains("GA2zxWdhAYtREeYCVFTGRhHQmYMPAP"));
    }

    #[wasm_bindgen_test]
    async fn test_encrypt_credentials() {
        let encrypted = super::encrypt_credentials("userleo456", "userleo456");
        assert_eq!(encrypted, "a3c7f7de9a34bcab4554f7cedf6046e041eeb3a9211466d92ecaa9763ac3557b:f3ac0f33fe97412a39ebb5d11d111961a754ecbbbdf12c71342adb7022ae3a2d");
    }

    #[wasm_bindgen_test]
    async fn test_amp0ext() {
        let mut amp0 = Amp0Ext::new_mainnet("userleo456", "userleo456", "")
            .await
            .unwrap();
        let last_index = amp0.last_index();
        assert!(last_index > 20);

        let addr = amp0.address(None).await.unwrap();
        assert_eq!(addr.index(), last_index + 1);
        assert_eq!(amp0.last_index(), last_index + 1);

        // Sync the wollet
        let mut wollet = amp0.wollet().unwrap();

        let network = Network::mainnet();
        let mut client = network.default_esplora_client();
        let update = client.full_scan(&wollet).await.unwrap().unwrap();

        wollet.apply_update(&update).unwrap();
        let balance = wollet.balance().unwrap();
        use std::collections::HashMap;
        let balance: HashMap<lwk_wollet::elements::AssetId, u64> =
            serde_wasm_bindgen::from_value(balance).unwrap();
        let lbtc = lwk_wollet::ElementsNetwork::Liquid.policy_asset();
        let mut expected = HashMap::new();
        expected.insert(lbtc, 0);
        assert_eq!(balance, expected);
    }
}
