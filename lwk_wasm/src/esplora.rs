use crate::{Error, Network, Pset, Txid, Update, Wollet};
use wasm_bindgen::prelude::*;

/// Wrapper of [`lwk_wollet::EsploraWasmClient`]
#[wasm_bindgen]
pub struct EsploraClient {
    inner: lwk_wollet::EsploraWasmClient,
}

#[wasm_bindgen]
impl EsploraClient {
    /// Creates a client, wrapper of [`lwk_wollet::EsploraWasmClient`]
    #[wasm_bindgen(constructor)]
    pub fn new(network: &Network, url: &str, waterfalls: bool) -> Self {
        let inner = lwk_wollet::EsploraWasmClient::new((*network).into(), url, waterfalls);
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
}

#[cfg(test)]
mod tests {

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_sleep() {
        lwk_wollet::async_sleep(1).await;
    }
}
