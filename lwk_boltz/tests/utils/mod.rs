use base64::{engine::general_purpose::STANDARD, Engine as _};
use boltz_client::{
    boltz::BoltzWsApi,
    network::{BitcoinChain, Chain, LiquidChain},
};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use futures::FutureExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::{error::Error, sync::Arc};
use tokio::task::JoinHandle;

const BITCOIND_URL: &str = "http://localhost:18443/wallet/client";
const ELEMENTSD_URL: &str = "http://localhost:18884/wallet/client";
const LND_URL: &str = "https://localhost:8081";

const PROXY_URL: &str = "http://localhost:51234/proxy";

const BITCOIND_COOKIE: Option<&str> = option_env!("BITCOIND_COOKIE");
const ELEMENTSD_COOKIE: &str = "regtest:regtest";
const LND_MACAROON_HEX: Option<&str> = option_env!("LND_MACAROON_HEX");

pub(crate) const WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(10);
pub(crate) const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
pub(crate) const DEFAULT_REGTEST_NODE: &str = "localhost:19002";
pub(crate) const BOLTZ_REGTEST: &str = "http://localhost:9001/v2";

async fn json_rpc_request(
    chain: Chain,
    method: &str,
    params: Value,
) -> Result<Value, Box<dyn Error>> {
    let (url, cookie) = match chain {
        Chain::Bitcoin(_) => (BITCOIND_URL, BITCOIND_COOKIE.unwrap()),
        Chain::Liquid(_) => (ELEMENTSD_URL, ELEMENTSD_COOKIE),
    };

    let client = Client::new();

    let req_body = json!({
        "jsonrpc": "1.0",
        "id": "curltest",
        "method": method,
        "params": params
    });

    let res = client
        .post(PROXY_URL)
        .header(
            "Authorization",
            format!("Basic {}", STANDARD.encode(cookie)),
        )
        .header("X-Proxy-URL", url)
        .json(&req_body)
        .send()
        .await?
        .json::<Value>()
        .await?;

    res.get("result")
        .cloned()
        .ok_or_else(|| "Invalid response".into())
}

async fn lnd_request(method: &str, params: Value) -> Result<Value, Box<dyn Error>> {
    let client = Client::new();
    let url = format!("{LND_URL}/{method}");

    let res = client
        .post(PROXY_URL)
        .header("Grpc-Metadata-macaroon", LND_MACAROON_HEX.unwrap())
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

pub async fn generate_address(chain: Chain) -> Result<String, Box<dyn Error>> {
    json_rpc_request(chain, "getnewaddress", json!([]))
        .await?
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid response".into())
}

pub async fn send_to_address(
    chain: Chain,
    address: &str,
    sat_amount: u64,
) -> Result<String, Box<dyn Error>> {
    let btc_amount = (sat_amount as f64) / 100_000_000.0;
    let params = json!([address, format!("{:.8}", btc_amount)]);
    json_rpc_request(chain, "sendtoaddress", params)
        .await?
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid response".into())
}

pub async fn generate_invoice_lnd(amount_sat: u64) -> Result<String, Box<dyn Error>> {
    let response = lnd_request("v1/invoices", json!({ "value": amount_sat })).await?;
    response["payment_request"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing payment_request field".into())
}

pub async fn pay_invoice_lnd_inner(invoice: &str) -> Result<(), Box<dyn Error>> {
    lnd_request(
        "v2/router/send",
        json!({ "payment_request": invoice, "timeout_seconds": 1 }),
    )
    .await?;
    Ok(())
}

pub fn start_ws(ws: Arc<BoltzWsApi>) {
    let future = ws.run_ws_loop();

    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    {
        tokio::spawn(future);
    }

    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        // In WASM, we can use spawn_local since we don't need Send
        wasm_bindgen_futures::spawn_local(future);
    }
}

pub fn start_pay_invoice_lnd(invoice: String) {
    let task = async move {
        pay_invoice_lnd_inner(&invoice).await.unwrap();
    };

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        tokio::spawn(task);
    }

    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        wasm_bindgen_futures::spawn_local(async {
            let timeout_future = gloo_timers::future::TimeoutFuture::new(5000);
            let _ = futures::select! {
                _ = task.fuse() => {},
                _ = timeout_future.fuse() => {},
            };
        });
    }
}

pub async fn mine_blocks(n_blocks: u64) -> Result<(), Box<dyn Error>> {
    for chain in [
        BitcoinChain::BitcoinRegtest.into(),
        LiquidChain::LiquidRegtest.into(),
    ] {
        let address = generate_address(chain).await?;
        json_rpc_request(chain, "generatetoaddress", json!([n_blocks, address])).await?;
    }
    Ok(())
}

pub fn start_block_mining() -> JoinHandle<()> {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            interval.tick().await;
            if let Err(e) = mine_blocks(1).await {
                log::error!("Failed to mine block: {:?}", e);
            } else {
                log::info!("Mined a block");
            }
        }
    })
}
