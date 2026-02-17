#![allow(dead_code)]
use base64::{engine::general_purpose::STANDARD, Engine as _};
use boltz_client::{
    boltz::{BoltzWsApi, SwapStatus},
    network::{BitcoinChain, Chain, LiquidChain},
    util::sleep,
};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use futures::FutureExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::{error::Error, sync::Arc};
use tokio::sync::broadcast::Receiver;
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
        .header(
            "Grpc-Metadata-macaroon",
            LND_MACAROON_HEX.expect("LND_MACAROON_HEX is not set"),
        )
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

/// Get the total balance at a specific address by querying listunspent
///
/// Returns the total amount in satoshis of all unspent outputs at the given address.
///
/// This function polls for UTXOs with a retry mechanism: it will attempt to find
/// at least one UTXO at the address up to 30 times, sleeping 1 second between attempts.
/// This is useful after a transaction has been broadcast but may not yet be visible
/// in the node's UTXO set. If no UTXOs are found after 30 attempts, returns an error.
pub async fn get_address_balance(chain: Chain, address: &str) -> Result<u64, Box<dyn Error>> {
    let params = json!([0, 9999999, [address]]);

    for attempt in 1..=30 {
        let result = json_rpc_request(chain, "listunspent", params.clone()).await?;
        let utxos = result.as_array().ok_or("Expected array response")?;

        if !utxos.is_empty() {
            let mut total_sats: u64 = 0;
            for utxo in utxos {
                // Amount is in BTC, convert to sats
                let amount_btc = utxo["amount"].as_f64().ok_or("Missing amount field")?;
                let amount_sats = (amount_btc * 100_000_000.0).round() as u64;
                total_sats += amount_sats;
            }
            log::info!(
                "Found {} UTXO(s) at {} after {} attempt(s), total: {} sats",
                utxos.len(),
                address,
                attempt,
                total_sats
            );
            return Ok(total_sats);
        }

        log::debug!("No UTXOs found at {address} (attempt {attempt}/30), sleeping 1s...",);
        sleep(std::time::Duration::from_secs(1)).await;
    }

    Err(format!("No UTXOs found at {address} after 30 attempts").into())
}

pub fn start_block_mining() -> JoinHandle<()> {
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            interval.tick().await;
            if let Err(e) = mine_blocks(1).await {
                log::error!("Failed to mine block: {e:?}");
            } else {
                log::info!("Mined a block");
            }
        }
    })
}

pub async fn next_status(
    updates: &mut Receiver<SwapStatus>,
    expected_status: &str,
) -> Result<boltz_client::boltz::SwapStatus, Box<dyn Error>> {
    tokio::select! {
        result = async {
            loop {
                let update = updates.recv().await?;
                log::info!("Waiting for status: {}", update.status);
                if update.status == expected_status {
                    return Ok(update);
                }
            }
        } => result,
        _ = sleep(WAIT_TIME) => {
            Err("Timeout waiting for status: {expected_status}".into())
        }
    }
}

pub async fn assert_next_continue_status(
    response: &mut lwk_boltz::LockupResponse,
    expected_status: &str,
) {
    match response.advance().await.expect("advance should not fail") {
        std::ops::ControlFlow::Continue(update) => {
            assert_eq!(
                update.status, expected_status,
                "Expected update status '{expected_status}', got '{}'",
                update.status
            );
        }
        std::ops::ControlFlow::Break(result) => {
            panic!("Expected status '{expected_status}', swap terminated with result: {result}")
        }
    }
}
