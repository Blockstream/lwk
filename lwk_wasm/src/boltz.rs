use std::{fmt, str::FromStr, time::Duration};

use wasm_bindgen::prelude::*;

use crate::{Address, Error, Network};

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
    pub fn new(
        network: &Network,
        esplora_client: &crate::EsploraClient,
    ) -> Result<BoltzSessionBuilder, crate::Error> {
        let async_client = esplora_client.clone_async_client()?;
        let boltz_client = lwk_boltz::clients::EsploraClient::from_client(
            std::sync::Arc::new(async_client),
            network.into(),
        );
        let client = lwk_boltz::clients::AnyClient::Esplora(std::sync::Arc::new(boltz_client));

        Ok(lwk_boltz::BoltzSession::builder(network.into(), client).into())
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
    /// Avoid a call to the boltz API to recover this information.
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
        Ok(self
            .inner
            .bitcoin_electrum_client(bitcoin_electrum_client)
            .map(Into::into)?)
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
        let inner = self.inner.build().await?;
        Ok(BoltzSession { inner })
    }
}

#[wasm_bindgen]
#[derive(Debug)]
pub struct PreparePayResponse {
    inner: lwk_boltz::PreparePayResponse,
}

impl From<lwk_boltz::PreparePayResponse> for PreparePayResponse {
    fn from(inner: lwk_boltz::PreparePayResponse) -> Self {
        Self { inner }
    }
}

impl From<PreparePayResponse> for lwk_boltz::PreparePayResponse {
    fn from(wrapper: PreparePayResponse) -> Self {
        wrapper.inner
    }
}

#[wasm_bindgen]
impl PreparePayResponse {
    /// Serialize the response to JSON string for JS interop
    pub fn serialize(&self) -> Result<String, Error> {
        Ok(self.inner.serialize()?)
    }

    pub fn swap_id(&self) -> String {
        self.inner.swap_id().to_string()
    }

    pub fn uri(&self) -> String {
        self.inner.uri().to_string()
    }

    pub fn uri_address(&self) -> Result<Address, Error> {
        Ok(self.inner.uri_address()?.into())
    }

    pub fn uri_amount(&self) -> u64 {
        self.inner.uri_amount()
    }

    pub async fn complete_pay(self) -> Result<bool, Error> {
        Ok(self.inner.complete_pay().await?)
    }
}

/// Wrapper over [`lwk_boltz::InvoiceResponse`]
#[wasm_bindgen]
#[derive(Debug)]
pub struct InvoiceResponse {
    inner: lwk_boltz::InvoiceResponse,
}

impl From<lwk_boltz::InvoiceResponse> for InvoiceResponse {
    fn from(inner: lwk_boltz::InvoiceResponse) -> Self {
        Self { inner }
    }
}

impl From<InvoiceResponse> for lwk_boltz::InvoiceResponse {
    fn from(wrapper: InvoiceResponse) -> Self {
        wrapper.inner
    }
}

#[wasm_bindgen]
impl InvoiceResponse {
    /// Serialize the response to JSON string for JS interop
    pub fn serialize(&self) -> Result<String, Error> {
        Ok(self.inner.serialize()?)
    }

    /// Return the bolt11 invoice string
    #[wasm_bindgen(js_name = bolt11Invoice)]
    pub fn bolt11_invoice(&self) -> String {
        self.inner.bolt11_invoice().to_string()
    }

    /// Complete the payment by advancing through the swap states until completion or failure
    /// Consumes self as the inner method does
    pub async fn complete_pay(self) -> Result<bool, Error> {
        Ok(self.inner.complete_pay().await?)
    }
}

#[wasm_bindgen]
impl BoltzSession {
    /// Get the rescue file
    pub fn rescue_file(&self) -> Result<String, Error> {
        let r = self.inner.rescue_file();
        Ok(serde_json::to_string(&r)?)
    }

    /// Prepare a lightning invoice payment
    pub async fn prepare_pay(
        &self,
        lightning_payment: &LightningPayment,
        refund_address: &Address,
    ) -> Result<PreparePayResponse, Error> {
        let r = self
            .inner
            .prepare_pay(&lightning_payment.inner, refund_address.as_ref(), None)
            .await?;
        Ok(r.into())
    }

    /// Create a lightning invoice for receiving payment
    pub async fn invoice(
        &self,
        amount: u64,
        description: Option<String>,
        claim_address: &Address,
    ) -> Result<InvoiceResponse, Error> {
        let r = self
            .inner
            .invoice(amount, description, claim_address.as_ref(), None)
            .await?;
        Ok(r.into())
    }
}

/// Wrapper over [`lwk_boltz::LightningPayment`]
#[wasm_bindgen]
#[derive(Debug)]
pub struct LightningPayment {
    inner: lwk_boltz::LightningPayment,
}

impl fmt::Display for LightningPayment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl From<lwk_boltz::LightningPayment> for LightningPayment {
    fn from(inner: lwk_boltz::LightningPayment) -> Self {
        Self { inner }
    }
}

impl From<LightningPayment> for lwk_boltz::LightningPayment {
    fn from(wrapper: LightningPayment) -> Self {
        wrapper.inner
    }
}

#[wasm_bindgen]
impl LightningPayment {
    /// Create a LightningPayment from a bolt11 invoice string
    #[wasm_bindgen(constructor)]
    pub fn new(invoice: &str) -> Result<LightningPayment, Error> {
        let payment = lwk_boltz::LightningPayment::from_str(invoice)
            .map_err(|(e1, e2)| Error::Generic(format!("{e1:?}, {e2:?}")))?;
        Ok(payment.into())
    }

    /// Return a string representation of the LightningPayment
    #[wasm_bindgen(js_name = toString)]
    pub fn to_string_js(&self) -> String {
        format!("{self}")
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use wasm_bindgen_test::*;

    use crate::{
        Address, BoltzSessionBuilder, EsploraClient, LightningPayment, Mnemonic, Network, Signer,
        TxBuilder, Wollet,
    };

    use lwk_wollet::asyncr::async_sleep;
    use reqwest::Client;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::error::Error;

    wasm_bindgen_test_configure!(run_in_browser);

    const LND_URL: &str = "https://localhost:8081";

    const PROXY_URL: &str = "http://localhost:51234/proxy";

    #[wasm_bindgen_test]
    async fn test_boltz_session_builder() {
        let network = Network::mainnet();
        let client = network.default_esplora_client();
        let builder = BoltzSessionBuilder::new(&network, &client).unwrap();
        let session = builder.build().await.unwrap();
        let rescue_file = session.rescue_file().unwrap();
        assert_ne!(rescue_file, "");
        let address = "lq1qqvp9g33gw9y05xava3dvcpq8pnkv82yj3tdnzp547eyp9yrztz2lkyxrhscd55ev4p7lj2n72jtkn5u4xnj4v577c42jhf3ww";
        let invoice_str = "lnbc2220n1p534hfqpp5kqs680arwtec67pcl2lq0mvvcyww056wkvrlsc3222qwez0x8lcqdquf35kw6r5de5kueeqwpshjmt9de6qcqzxrxqyp2xqrzjqgvw6stfqrph8t0qq6g5y0ut35cfxun2hzysmdskrdp9hdy6tvnvjzzxeyqq28qqqqqqqqqqqqqqq9gq2yrzjqtnpp8ds33zeg5a6cumptreev23g7pwlp39cvcz8jeuurayvrmvdsrw9ysqqq9gqqqqqqqqpqqqqq9sq2gsp5v724rcrc2puam2e9dy00qhvz3h5467he46eh75vx7fhm6skwqfus9qxpqysgqqa6fea42v5ttr84efdwndqcr3nyxe0pfegmu04xscrwcau5ufg4x7f6lvf9tre9w5t99xn2y8slvwasnaa2sk98rdyege5lec8u42qsq5nzzjs";
        let invoice = LightningPayment::new(invoice_str).unwrap();
        let address = Address::new(address).unwrap();
        let err = session.prepare_pay(&invoice, &address).await.unwrap_err();
        assert!(err.to_string().contains("magic routing hint"));
        let invoice_str = "lnbc2u1p534c9jsp5n6497xhz7a0c44elx56fajryf7lwrpuhh6mnmpxk2pasq7gvqx2spp5mmvw9lh8wwxl8zlqrmfwerc073cfr2y5qrtsldrczup77zx54m4sdqgd3skyetvxqyjw5qcqpjrzjqdx5l2zdly4gg6chmr4rypjvkrdmw6k9tfjxy75z05x0kxsya5xs2rwazuqq0egqqqqqqqlgqqqqqzsqyg9qxpqysgqrnk5e6n8rfam7cytfu46s3zh6uuyjy8mye94ks2du8asq53tv2erv93mnaqedcf0mhk2s9luea36we9950er8f646trk8vtqsfncdqsp0kun79";
        let invoice = LightningPayment::new(invoice_str).unwrap();
        let err = session.prepare_pay(&invoice, &address).await.unwrap_err();
        assert!(err
            .to_string()
            .contains("a swap with this invoice exists already"));

        let _invoice_response = session
            .invoice(1000, Some("test".to_string()), &address)
            .await
            .unwrap();
    }

    // #[ignore = "requires regtest environment"]
    #[wasm_bindgen_test]
    async fn test_boltz_submarine_reverse() {
        let network = Network::regtest_default();

        // Create a wpkh slip77 Wollet with the abandon mnemonic
        let mnemonic = Mnemonic::new("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about").unwrap();
        let signer = Signer::new(&mnemonic, &network).unwrap();
        let desc = signer.wpkh_slip77_descriptor().unwrap();
        let mut wollet = Wollet::new(&network, &desc).unwrap();

        let mut client =
            EsploraClient::new(&network, "http://127.0.0.1:4003/api/", false, 4, false).unwrap();

        // Create BoltzSession with the same Esplora client used for wallet scanning
        let builder = BoltzSessionBuilder::new(&network, &client).unwrap();
        let session = builder.build().await.unwrap();

        scan_wollet(&mut wollet, &mut client).await;
        let balance1 = lbtc_balance(&wollet);
        assert!(balance1 > 0);

        // Pay a lightning invoice
        let invoice_amount = 1000;
        let invoice = generate_invoice_lnd(invoice_amount).await.unwrap();
        assert!(invoice.starts_with("lnbcrt1"));
        let refund_address = wollet.address(None).unwrap();
        let invoice = LightningPayment::new(&invoice).unwrap();
        let invoice_response = session
            .prepare_pay(&invoice, &refund_address.address())
            .await
            .unwrap();
        let address = invoice_response.uri_address().unwrap();
        let amount = invoice_response.uri_amount();
        assert!(address.to_string().starts_with("el1"));
        assert!(amount > invoice_amount);

        // Create a transaction to send the amount to the address
        let mut builder = TxBuilder::new(&network);
        builder = builder.add_lbtc_recipient(&address, amount).unwrap();
        let mut pset = builder.finish(&wollet).unwrap();

        // Sign the transaction
        pset = signer.sign(pset).unwrap();

        // Finalize the transaction
        pset = wollet.finalize(pset).unwrap();

        // Extract and broadcast the transaction
        let tx = pset.extract_tx().unwrap();
        let txid = client.broadcast_tx(&tx).await.unwrap();

        // Optionally apply the transaction to the wallet
        wollet.apply_transaction(&tx).unwrap();

        // Verify the transaction was broadcast
        assert!(!txid.to_string().is_empty());

        let result = invoice_response.complete_pay().await.unwrap();
        assert!(result);

        // Receive a lightning payment
        scan_wollet(&mut wollet, &mut client).await;
        let balance2 = lbtc_balance(&wollet);
        assert!(balance2 < balance1);
        let claim_address = wollet.address(None).unwrap();
        let invoice = session
            .invoice(1000, Some("test".to_string()), &claim_address.address())
            .await
            .unwrap();
        pay_invoice_lnd(&invoice.bolt11_invoice()).await.unwrap();
        let result = invoice.complete_pay().await.unwrap();
        assert!(result);
        let wait_secs = 20;
        for i in 0..wait_secs {
            async_sleep(1_000).await;
            scan_wollet(&mut wollet, &mut client).await;
            let balance3 = lbtc_balance(&wollet);
            if balance3 > balance2 {
                break;
            }
            assert!(
                i < wait_secs,
                "Balance did not increase after {wait_secs} seconds"
            );
        }
    }

    async fn scan_wollet(wollet: &mut Wollet, client: &mut EsploraClient) {
        let update = client.full_scan(wollet).await.unwrap();
        if let Some(update) = update {
            wollet.apply_update(&update).unwrap();
        }
    }

    fn lbtc_balance(wollet: &Wollet) -> u64 {
        let network = Network::regtest_default();
        let balance = wollet.balance().unwrap();
        let balance: HashMap<lwk_wollet::elements::AssetId, u64> =
            serde_wasm_bindgen::from_value(balance.entries().unwrap()).unwrap();
        let policy_asset = network.policy_asset().into();
        *balance.get(&policy_asset).unwrap_or(&0)
    }

    // copied from lwk_boltz::tests::utils::lnd_request
    async fn lnd_request(method: &str, params: Value) -> Result<Value, Box<dyn Error>> {
        let client = Client::new();
        let url = format!("{LND_URL}/{method}");

        // can't use option_env!("LND_MACAROON_HEX") because it's in lwk_boltz crate.
        // just reading .env which is expected when the boltz regtest is running
        let env = include_str!("../../lwk_boltz/.env");
        let mac = env
            .split("\n")
            .find(|line| line.starts_with("LND_MACAROON_HEX="))
            .unwrap()
            .split("=")
            .nth(1)
            .unwrap();

        let res = client
            .post(PROXY_URL)
            .header("Grpc-Metadata-macaroon", mac)
            .header("X-Proxy-URL", url)
            .json(&params)
            .send()
            .await?
            .text()
            .await?;

        // Parse the last JSON in the response (multiple JSONs separated by newlines)
        let last_json_line = res
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .ok_or("Empty response")?;

        let parsed: Value = serde_json::from_str(last_json_line)?;
        Ok(parsed)
    }

    pub async fn generate_invoice_lnd(amount_sat: u64) -> Result<String, Box<dyn Error>> {
        let response = lnd_request("v1/invoices", json!({ "value": amount_sat })).await?;
        response["payment_request"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing payment_request field".into())
    }

    pub async fn pay_invoice_lnd(invoice: &str) -> Result<(), Box<dyn Error>> {
        let invoice = invoice.to_string();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = lnd_request(
                "v2/router/send",
                json!({ "payment_request": invoice, "timeout_seconds": 1 }),
            )
            .await
            .unwrap();
        });
        Ok(())
    }
}
