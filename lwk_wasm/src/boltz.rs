use std::{str::FromStr, time::Duration};

use wasm_bindgen::prelude::*;

use crate::{Error, Network};

/// Wrapper over [`lwk_boltz::BoltzSessionBuilder`]
#[wasm_bindgen]
pub struct BoltzSessionBuilder {
    inner: lwk_boltz::BoltzSessionBuilder,
}

impl From<lwk_boltz::BoltzSessionBuilder> for BoltzSessionBuilder {
    fn from(inner: lwk_boltz::BoltzSessionBuilder) -> Self {
        Self { inner }
    }
}

impl From<BoltzSessionBuilder> for lwk_boltz::BoltzSessionBuilder {
    fn from(builder: BoltzSessionBuilder) -> Self {
        builder.inner
    }
}

/// Wrapper over [`lwk_boltz::BoltzSession`]
#[wasm_bindgen]
pub struct BoltzSession {
    inner: lwk_boltz::BoltzSession,
}

#[wasm_bindgen]
impl BoltzSessionBuilder {
    /// Create a new BoltzSessionBuilder with the given network
    ///
    /// This creates a builder with default Esplora client for the network.
    #[wasm_bindgen(constructor)]
    pub fn new(network: &Network) -> BoltzSessionBuilder {
        // Create an EsploraClient for the network using the same URLs as default_esplora_client
        let url = if network.is_mainnet() {
            "https://blockstream.info/liquid/api"
        } else if network.is_testnet() {
            "https://blockstream.info/liquidtestnet/api"
        } else {
            "http://127.0.0.1:3000"
        };

        let esplora_client = lwk_boltz::clients::EsploraClient::new(url, network.into());
        let client = lwk_boltz::clients::AnyClient::Esplora(std::sync::Arc::new(esplora_client));

        lwk_boltz::BoltzSession::builder(network.into(), client).into()
    }

    /// Set the timeout for creating swaps
    ///
    /// If not set, the default timeout of 10 seconds is used.
    #[wasm_bindgen(js_name = createSwapTimeout)]
    pub fn create_swap_timeout(self, timeout_seconds: u64) -> BoltzSessionBuilder {
        self.inner
            .create_swap_timeout(Duration::from_secs(timeout_seconds))
            .into()
    }

    /// Set the timeout for the advance call
    ///
    /// If not set, the default timeout of 3 minutes is used.
    #[wasm_bindgen(js_name = timeoutAdvance)]
    pub fn timeout_advance(self, timeout_seconds: u64) -> BoltzSessionBuilder {
        self.inner
            .timeout_advance(Duration::from_secs(timeout_seconds))
            .into()
    }

    /// Set the mnemonic for deriving swap keys
    ///
    /// If not set, a new random mnemonic will be generated.
    #[wasm_bindgen]
    pub fn mnemonic(self, mnemonic: &crate::Mnemonic) -> BoltzSessionBuilder {
        self.inner.mnemonic(mnemonic.into()).into()
    }

    /// Set the polling flag
    ///
    /// If true, the advance call will not await on the websocket connection returning immediately
    /// even if there is no update, thus requiring the caller to poll for updates.
    ///
    /// If true, the timeout_advance will be ignored even if set.
    #[wasm_bindgen]
    pub fn polling(self, polling: bool) -> BoltzSessionBuilder {
        self.inner.polling(polling).into()
    }

    /// Set the next index to use for deriving keypairs
    ///
    /// Should be always set when reusing a mnemonic to avoid abusing the boltz API to recover
    /// this information.
    ///
    /// When the mnemonic is not set, this is ignored.
    #[wasm_bindgen(js_name = nextIndexToUse)]
    pub fn next_index_to_use(self, next_index_to_use: u32) -> BoltzSessionBuilder {
        self.inner.next_index_to_use(next_index_to_use).into()
    }

    /// Set the referral id for the BoltzSession
    #[wasm_bindgen(js_name = referralId)]
    pub fn referral_id(self, referral_id: &str) -> BoltzSessionBuilder {
        self.inner.referral_id(referral_id.to_string()).into()
    }

    /// Set the url of the bitcoin electrum client
    #[wasm_bindgen(js_name = bitcoinElectrumClient)]
    pub fn bitcoin_electrum_client(
        self,
        bitcoin_electrum_client: &str,
    ) -> Result<BoltzSessionBuilder, Error> {
        self.inner
            .bitcoin_electrum_client(bitcoin_electrum_client)
            .map_err(|e| Error::Generic(e.to_string()))
            .map(Into::into)
    }

    /// Set the random preimages flag
    ///
    /// The default is false, the preimages will be deterministic and the rescue file will be
    /// compatible with the Boltz web app.
    /// If true, the preimages will be random potentially allowing concurrent sessions with the same
    /// mnemonic, but completing the swap will be possible only with the preimage data. For example
    /// the boltz web app will be able only to refund the swap, not to bring it to completion.
    /// If true, when serializing the swap data, the preimage will be saved in the data.
    #[wasm_bindgen(js_name = randomPreimages)]
    pub fn random_preimages(self, random_preimages: bool) -> BoltzSessionBuilder {
        self.inner.random_preimages(random_preimages).into()
    }

    /// Build the BoltzSession
    #[wasm_bindgen]
    pub async fn build(self) -> Result<BoltzSession, Error> {
        let inner = self
            .inner
            .build()
            .await
            .map_err(|e| Error::Generic(e.to_string()))?;
        Ok(BoltzSession { inner })
    }
}

impl BoltzSession {
    /// Get the rescue file
    pub fn rescue_file(&self) -> Result<String, Error> {
        let r = self.inner.rescue_file();
        Ok(serde_json::to_string(&r).map_err(|e| Error::Generic(e.to_string()))?)
    }

    /// Prepare a lightning invoice payment
    pub async fn prepare_pay(&self, invoice: &str, refund_address: &str) -> Result<String, Error> {
        let refund_address = lwk_wollet::elements::Address::from_str(refund_address)
            .map_err(|e| Error::Generic(e.to_string()))?;
        let lightning_payment = lwk_boltz::LightningPayment::Bolt11(Box::new(
            lwk_boltz::Bolt11Invoice::from_str(invoice)
                .map_err(|e| Error::Generic(e.to_string()))?,
        ));
        let r = self
            .inner
            .prepare_pay(&lightning_payment, &refund_address, None)
            .await
            .map_err(|e| Error::Generic(e.to_string()))?;
        // TODO this method should return a PreparePayResponse object
        Ok(r.serialize().map_err(|e| Error::Generic(e.to_string()))?)
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::*;

    use crate::{BoltzSessionBuilder, Network};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_boltz_session_builder() {
        let network = Network::mainnet();
        let builder = BoltzSessionBuilder::new(&network);
        let session = builder.build().await.unwrap();
        let rescue_file = session.rescue_file().unwrap();
        assert_ne!(rescue_file, "");
        let err = session.prepare_pay("lnbc2220n1p534hfqpp5kqs680arwtec67pcl2lq0mvvcyww056wkvrlsc3222qwez0x8lcqdquf35kw6r5de5kueeqwpshjmt9de6qcqzxrxqyp2xqrzjqgvw6stfqrph8t0qq6g5y0ut35cfxun2hzysmdskrdp9hdy6tvnvjzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2gsp5v724rcrc2puam2e9dy00qhvz3h5467he46eh75vx7fhm6skwqfus9qxpqysgqqa6fea42v5ttr84efdwndqcr3nyxe0pfegmu04xscrwcau5ufg4x7f6lvf9tre9w5t99xn2y8slvwasnaa2sk98rdyege5lec8u42qsq5nzzjs", "lq1qqvp9g33gw9y05xava3dvcpq8pnkv82yj3tdnzp547eyp9yrztz2lkyxrhscd55ev4p7lj2n72jtkn5u4xnj4v577c42jhf3ww").await.unwrap_err();
        assert!(err.to_string().contains("magic routing hint"));
        let err = session.prepare_pay("lnbc2u1p534c9jsp5n6497xhz7a0c44elx56fajryf7lwrpuhh6mnmpxk2pasq7gvqx2spp5mmvw9lh8wwxl8zlqrmfwerc073cfr2y5qrtsldrczup77zx54m4sdqgd3skyetvxqyjw5qcqpjrzjqdx5l2zdly4gg6chmr4rypjvkrdmw6k9tfjxy75z05x0kxsya5xs2rwazuqq0egqqqqqqqlgqqqqqzsqyg9qxpqysgqrnk5e6n8rfam7cytfu46s3zh6uuyjy8mye94ks2du8asq53tv2erv93mnaqedcf0mhk2s9luea36we9950er8f646trk8vtqsfncdqsp0kun79", "lq1qqvp9g33gw9y05xava3dvcpq8pnkv82yj3tdnzp547eyp9yrztz2lkyxrhscd55ev4p7lj2n72jtkn5u4xnj4v577c42jhf3ww").await.unwrap_err();
        assert!(err
            .to_string()
            .contains("a swap with this invoice exists already"));
    }
}
