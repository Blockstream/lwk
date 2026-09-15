use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use std::thread::JoinHandle;

use clap::Parser;
use elements::{Address, Txid};
use serde_json::Value;
use tempfile::TempDir;

use lwk_cli::{inner_main, Cli};
use lwk_test_util::TestEnv;

/// Returns a non-used local port if available.
///
/// Note there is a race condition during the time the method check availability and the caller
pub fn get_available_addr() -> anyhow::Result<SocketAddr> {
    // using 0 as port let the system assign a port available
    let t = std::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0))?;
    Ok(t.local_addr()?)
}

pub fn get_balance(cli: &str, wallet: &str, asset: &str) -> u64 {
    sh(&format!("{cli} server scan"));
    let r = sh(&format!("{cli} wallet balance --wallet {wallet}"));
    let b = r.get("balance").unwrap().as_object().unwrap();
    b.get(asset).unwrap().as_u64().unwrap()
}

#[track_caller]
pub fn sh_result(command: &str) -> anyhow::Result<Value> {
    let shell_words = shellwords::split(command).unwrap();
    let cli = Cli::try_parse_from(shell_words).unwrap();
    // cli.network = Network::Regtest;
    inner_main(cli)
}

#[track_caller]
pub fn sh(command: &str) -> Value {
    sh_result(command).unwrap()
}

pub fn sh_err(command: &str) -> String {
    format!("{:?}", sh_result(command).unwrap_err())
}

pub fn setup_cli(env: TestEnv) -> (JoinHandle<()>, TempDir, String, String, TestEnv) {
    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();

    let mut registry_url = env.registry_url();
    if !registry_url.is_empty() {
        registry_url = format!("--registry-url {registry_url}");
    }

    let server_url = format!("--server-url {}", &env.electrum_url());
    let addr = get_available_addr().unwrap();

    let cli = format!("cli --addr {addr} -n regtest --datadir {datadir}");
    let params = format!("{server_url} {registry_url}");

    let t = {
        let cli = cli.clone();
        let params = params.clone();
        std::thread::spawn(move || {
            sh(&format!(
                "{cli} server start --scanning-interval 1 {params}"
            ));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    (t, tmp, cli, params, env)
}

pub fn get_str<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).unwrap().as_str().unwrap()
}

pub fn get_len(v: &Value, key: &str) -> usize {
    v.get(key).unwrap().as_array().unwrap().len()
}

pub fn get_desc(r: &Value) -> String {
    let desc = get_str(r, "descriptor");
    // The returned descriptor is equivalent but it could be slightly different
    let desc = desc.replace('\'', "h");
    // Changing the descriptor string invalidates the checksum
    remove_checksum(&desc)
}

pub fn remove_checksum(desc: &str) -> String {
    desc.split('#')
        .collect::<Vec<_>>()
        .first()
        .unwrap()
        .to_string()
}

pub fn sw_signer(cli: &str, name: &str) {
    let r = sh(&format!("{cli} signer generate"));
    let mnemonic = get_str(&r, "mnemonic");
    sh(&format!(
        "{cli} signer load-software --persist true --mnemonic \"{mnemonic}\" --signer {name}"
    ));
}

pub fn keyorigin(cli: &str, signer: &str, bip: &str) -> String {
    let r = sh(&format!("{cli} signer xpub --signer {signer} --kind {bip}"));
    get_str(&r, "keyorigin_xpub").to_string()
}

pub fn multisig_wallet(cli: &str, name: &str, threshold: u32, signers: &[&str], dbk: &str) {
    let xpubs = signers
        .iter()
        .map(|s| format!(" --keyorigin-xpub {}", keyorigin(cli, s, "bip87")))
        .collect::<Vec<_>>()
        .join("");
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key {dbk} --kind wsh --threshold {threshold}{xpubs}"));
    let d = get_str(&r, "descriptor");
    sh(&format!("{cli} wallet load --wallet {name} -d {d}"));
    for signer in signers {
        sh(&format!(
            "{cli} signer register-multisig --signer {signer} --wallet {name}"
        ));
    }
}

pub fn singlesig_wallet(cli: &str, wallet: &str, signer: &str, dbk: &str, kind: &str) {
    let r = sh(&format!(
        "{cli} signer singlesig-desc -s {signer} --descriptor-blinding-key {dbk} --kind {kind}"
    ));
    let desc = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load -w {wallet} -d {desc}"));
}

pub fn txs(cli: &str, wallet: &str) -> Vec<Value> {
    let r = sh(&format!("{cli} wallet txs --wallet {wallet}"));
    r.get("txs").unwrap().as_array().unwrap().to_vec()
}

pub fn tx(cli: &str, wallet: &str, txid: &str) -> Option<Value> {
    txs(cli, wallet)
        .into_iter()
        .find(|tx| get_str(tx, "txid") == txid)
}

pub fn tx_details(cli: &str, wallet: &str, txid: &str) -> Value {
    sh(&format!("{cli} wallet tx-details -w {wallet} -t {txid}"))
}

pub fn tx_memo(cli: &str, wallet: &str, txid: &str) -> String {
    get_str(&tx(cli, wallet, txid).unwrap(), "memo").to_string()
}

pub fn wait_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

pub fn wait_tx(cli: &str, wallet: &str, txid: &str) {
    sh(&format!("{cli} server scan"));
    let ms = 500;
    let times = 20;
    for _ in 0..times {
        wait_ms(ms);
        if tx(cli, wallet, txid).is_some() {
            return;
        }
    }
    panic!("Waited tx {txid} for {}s", ms * times / 1000)
}

pub fn address(cli: &str, wallet: &str) -> String {
    let r = sh(&format!("{cli} wallet address --wallet {wallet}"));
    get_str(&r, "address").to_string()
}

pub fn addr_memo(cli: &str, w: &str, i: u32) -> String {
    let r = sh(&format!("{cli} wallet address --wallet {w} --index {i}"));
    get_str(&r, "memo").to_string()
}

pub fn asset_ids_from_issuance_pset(cli: &str, wallet: &str, pset: &str) -> (String, String) {
    let r = sh(&format!("{cli} wallet pset-details -w {wallet} -p {pset}"));
    let issuances = r.get("issuances").unwrap().as_array().unwrap();
    let asset = get_str(&issuances[0], "asset").to_string();
    let token = get_str(&issuances[0], "token").to_string();
    (asset, token)
}

pub fn fund(env: &TestEnv, cli: &str, wallet: &str, sats: u64) -> (Txid, Address) {
    let addr = Address::from_str(&address(cli, wallet)).unwrap();

    let txid = env.elementsd_sendtoaddress(&addr, sats, None);
    // Only 2 blocks are necessary to make coinbase spendable
    env.elementsd_generate(2);
    wait_tx(cli, wallet, &txid.to_string());
    (txid, addr)
}

pub fn complete(cli: &str, wallet: &str, pset: &str, signers: &[&str]) -> String {
    // Sign both serially and in parallel
    let pset = pset.to_string();
    let mut pset_serial = pset.to_string();
    let mut pset_args = "".to_string();
    for signer in signers {
        let r = sh(&format!(
            "{cli} signer sign -s {signer} --pset {pset_serial}"
        ));
        pset_serial = get_str(&r, "pset").to_string();
        let r = sh(&format!("{cli} signer sign -s {signer} --pset {pset}"));
        pset_args = format!("{pset_args} --pset {}", get_str(&r, "pset"));
    }
    let r = sh(&format!("{cli} wallet combine -w {wallet} {pset_args}"));
    let pset_combined = get_str(&r, "pset");
    // In general PSETs are not equal since order of keys and signatures might differ

    sh(&format!(
        "{cli} wallet broadcast -w {wallet} --pset {pset_serial} --dry-run"
    ));
    let r = sh(&format!(
        "{cli} wallet broadcast -w {wallet} --pset {pset_combined}"
    ));
    let txid = get_str(&r, "txid");
    wait_tx(cli, wallet, txid);
    txid.to_string()
}

pub fn send(
    cli: &str,
    wallet: &str,
    address: &str,
    asset: &str,
    sats: u64,
    signers: &[&str],
) -> String {
    let recipient = format!(" --recipient {address}:{sats}:{asset}");
    let r = sh(&format!("{cli} wallet send --wallet {wallet} {recipient}"));
    complete(cli, wallet, get_str(&r, "pset"), signers)
}
