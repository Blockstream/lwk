use crate::{Error, Network, Pset, Txid, Update, Wollet};
use lwk_wollet::{age, clients::asyncr};
use wasm_bindgen::prelude::*;

/// Wrapper of [`asyncr::EsploraClient`]
#[wasm_bindgen]
pub struct EsploraClient {
    inner: asyncr::EsploraClient,
}

#[wasm_bindgen]
impl EsploraClient {
    /// Creates a client, wrapper of [`asyncr::EsploraClient`]
    #[wasm_bindgen(constructor)]
    pub fn new(network: &Network, url: &str, waterfalls: bool) -> Self {
        let inner = asyncr::EsploraClient::new(network.into(), url, waterfalls);
        Self { inner }
    }

    #[wasm_bindgen(js_name = fullScan)]
    pub async fn full_scan(&mut self, wollet: &Wollet) -> Result<Option<Update>, Error> {
        let update: Option<lwk_wollet::Update> = self.inner.full_scan(wollet.as_ref()).await?;
        Ok(update.map(Into::into))
    }

    pub async fn broadcast(&mut self, pset: &Pset) -> Result<Txid, Error> {
        let tx = pset.extract_tx()?;
        let txid = self.inner.broadcast(&(tx.into())).await?;
        Ok(txid.into())
    }

    pub async fn set_waterfalls_server_recipient(&mut self, recipient: &str) -> Result<(), Error> {
        let recipient: age::x25519::Recipient = recipient
            .parse()
            .map_err(|e: &str| Error::Generic(e.to_string()))?;
        self.inner.set_waterfalls_server_recipient(recipient);
        Ok(())
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_sleep() {
        lwk_wollet::clients::asyncr::async_sleep(1).await;
    }
}
