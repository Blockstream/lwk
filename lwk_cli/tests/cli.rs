use std::{
    collections::HashSet,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    str::FromStr,
    thread::JoinHandle,
};

use clap::{Parser, ValueEnum};
use elements::{pset::PartiallySignedTransaction, Address};
use lwk_containers::{testcontainers::clients, JadeEmulator, EMULATOR_PORT};
use serde_json::Value;

use lwk_cli::{
    inner_main, AssetSubCommandsEnum, Cli, SignerSubCommandsEnum, WalletSubCommandsEnum,
};
use lwk_test_util::{setup, TestElectrumServer};
use tempfile::TempDir;

/// Returns a non-used local port if available.
///
/// Note there is a race condition during the time the method check availability and the caller
fn get_available_addr() -> anyhow::Result<SocketAddr> {
    // using 0 as port let the system assign a port available
    let t = std::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0))?;
    Ok(t.local_addr()?)
}

fn get_balance(cli: &str, wallet: &str, asset: &str) -> u64 {
    let r = sh(&format!("{cli} wallet balance --wallet {wallet}"));
    let b = r.get("balance").unwrap().as_object().unwrap();
    b.get(asset).unwrap().as_u64().unwrap()
}

#[track_caller]
fn sh_result(command: &str) -> anyhow::Result<Value> {
    let shell_words = shellwords::split(command).unwrap();
    let cli = Cli::try_parse_from(shell_words).unwrap();
    // cli.network = Network::Regtest;
    inner_main(cli)
}

#[track_caller]
pub fn sh(command: &str) -> Value {
    dbg!(command);
    sh_result(command).unwrap()
}

fn setup_cli() -> (JoinHandle<()>, TempDir, String, String, TestElectrumServer) {
    let server = setup(false);
    let electrum_url = &server.electrs.electrum_url;
    let addr = get_available_addr().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();
    let cli = format!("cli --addr {addr} -n regtest");
    let params = format!("--datadir {datadir} --electrum-url {electrum_url}");

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

    (t, tmp, cli, params, server)
}

fn get_str<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).unwrap().as_str().unwrap()
}

fn sw_signer(cli: &str, name: &str) {
    let r = sh(&format!("{cli} signer generate"));
    let mnemonic = get_str(&r, "mnemonic");
    sh(&format!(
        "{cli} signer load-software --mnemonic \"{mnemonic}\" --signer {name}"
    ));
}

fn keyorigin(cli: &str, signer: &str) -> String {
    let r = sh(&format!("{cli} signer xpub --signer {signer} --kind bip84"));
    get_str(&r, "keyorigin_xpub").to_string()
}

fn multisig_wallet(cli: &str, name: &str, threshold: u32, signers: &[&str], dbk: &str) {
    let xpubs = signers
        .iter()
        .map(|s| format!(" --keyorigin-xpub {}", keyorigin(cli, s)))
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

fn singlesig_wallet(cli: &str, wallet: &str, signer: &str, dbk: &str, kind: &str) {
    let r = sh(&format!(
        "{cli} signer singlesig-desc -s {signer} --descriptor-blinding-key {dbk} --kind {kind}"
    ));
    let desc = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load -w {wallet} -d {desc}"));
}

fn txs(cli: &str, wallet: &str) -> Vec<Value> {
    let r = sh(&format!("{cli} wallet txs --wallet {wallet}"));
    r.get("txs").unwrap().as_array().unwrap().to_vec()
}

fn tx(cli: &str, wallet: &str, txid: &str) -> Option<Value> {
    txs(cli, wallet)
        .into_iter()
        .find(|tx| get_str(tx, "txid") == txid)
}

fn wait_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

fn wait_tx(cli: &str, wallet: &str, txid: &str) {
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

fn address(cli: &str, wallet: &str) -> String {
    let r = sh(&format!("{cli} wallet address --wallet {wallet}"));
    get_str(&r, "address").to_string()
}

fn fund(server: &TestElectrumServer, cli: &str, wallet: &str, sats: u64) {
    let addr = Address::from_str(&address(cli, wallet)).unwrap();

    let txid = server.node_sendtoaddress(&addr, sats, None);
    // Only 2 blocks are necessary to make coinbase spendable
    server.generate(2);
    sh(&format!("{cli} server scan"));
    wait_tx(cli, wallet, &txid);
}

fn send(cli: &str, wallet: &str, address: &str, asset: &str, sats: u64, signers: &[&str]) {
    let recipient = format!(" --recipient {address}:{sats}:{asset}");
    let r = sh(&format!("{cli} wallet send --wallet {wallet} {recipient}"));
    let mut pset = get_str(&r, "pset").to_string();

    for signer in signers {
        let r = sh(&format!(
            "{cli} signer sign --signer {signer} --pset {pset}"
        ));
        pset = get_str(&r, "pset").to_string();
    }

    let r = sh(&format!(
        "{cli} wallet broadcast --wallet {wallet} --pset {pset}"
    ));
    let txid = get_str(&r, "txid");
    wait_tx(cli, wallet, txid);
}

#[test]
fn test_start_stop_persist() {
    let (t, _tmp, cli, params, _server) = setup_cli();

    let result = sh(&format!("{cli} signer list"));
    let signers = result.get("signers").unwrap();
    assert_eq!(signers.as_array().unwrap().len(), 0);

    let mnemonic = lwk_test_util::TEST_MNEMONIC;
    sh(&format!(
        r#"{cli} signer load-software --mnemonic "{mnemonic}" --signer s1"#
    ));
    let result = sh(&format!("{cli} signer generate"));
    let different_mnemonic = result.get("mnemonic").unwrap().as_str().unwrap();
    sh(&format!(
        r#"{cli} signer load-software --mnemonic "{different_mnemonic}" --signer s2"#,
    ));
    sh(&format!(r#"{cli} signer unload --signer s2"#)); // Verify unloads are handled

    sh(&format!(
        "{cli} signer load-external --fingerprint 11111111 --signer s2"
    ));
    sh(&format!(
        "{cli} signer load-jade --id 2111111111111111111111111111111111111112 --signer s3"
    ));

    let desc = "ct(c25deb86fa11e49d651d7eae27c220ef930fbd86ea023eebfa73e54875647963,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#q9cypnmc";
    sh(&format!("{cli} wallet load --wallet custody -d {desc}"));
    sh(&format!(r#"{cli} wallet unload --wallet custody"#)); // Verify unloads are handled
    sh(&format!("{cli} wallet load --wallet custody -d {desc}"));

    let contract = "{\"entity\":{\"domain\":\"tether.to\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Tether USD\",\"precision\":8,\"ticker\":\"USDt\",\"version\":0}";
    let asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
    let prev_txid = "9596d259270ef5bac0020435e6d859aea633409483ba64e232b8ba04ce288668";
    let prev_vout = 0;
    sh(&format!(
        "{cli} asset insert --asset {asset} --contract '{contract}' --prev-txid {prev_txid} --prev-vout {prev_vout}"
    ));

    let expected_signers = sh(&format!("{cli} signer list"));
    let r = expected_signers.get("signers").unwrap();
    assert_eq!(r.as_array().unwrap().len(), 3);

    let expected_wallets = sh(&format!("{cli} wallet list"));
    let r = expected_wallets.get("wallets").unwrap();
    assert_eq!(r.as_array().unwrap().len(), 1);

    let expected_assets = sh(&format!("{cli} asset list"));
    let r = expected_assets.get("assets").unwrap();
    assert_eq!(r.as_array().unwrap().len(), 3);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    let t = {
        let cli = cli.clone();
        let params = params.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(1000));

    let result = sh(&format!("{cli} signer list"));
    assert_eq!(expected_signers, result, "persist not working");

    let result = sh(&format!("{cli} wallet list"));
    assert_eq!(expected_wallets, result, "persist not working");

    let result = sh(&format!("{cli} asset list"));
    assert_eq!(expected_assets, result, "persist not working");

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // restarting another time to verify the initial load doesn't double the state
    let t = {
        let cli = cli.clone();
        let params = params.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(1000));

    let result = sh(&format!("{cli} signer list"));
    assert_eq!(expected_signers, result, "persist not working");

    let result = sh(&format!("{cli} wallet list"));
    assert_eq!(expected_wallets, result, "persist not working");

    let result = sh(&format!("{cli} asset list"));
    assert_eq!(expected_assets, result, "persist not working");

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_signer_load_unload_list() {
    let (t, _tmp, cli, _params, _server) = setup_cli();

    let result = sh(&format!("{cli} signer list"));
    let signers = result.get("signers").unwrap();
    assert!(signers.as_array().unwrap().is_empty());

    let mnemonic = lwk_test_util::TEST_MNEMONIC;
    let result = sh(&format!(
        r#"{cli} signer load-software --mnemonic "{mnemonic}" --signer ss "#
    ));
    assert_eq!(result.get("name").unwrap().as_str().unwrap(), "ss");

    let result = sh(&format!("{cli} signer generate"));
    let different_mnemonic = result.get("mnemonic").unwrap().as_str().unwrap();
    let result = sh_result(&format!(
        r#"{cli} signer load-software --mnemonic "{different_mnemonic}" --signer ss"#,
    ));
    assert!(format!("{:?}", result.unwrap_err()).contains("Signer 'ss' is already loaded"));

    let result = sh_result(&format!(
        r#"{cli} signer load-software --mnemonic "{mnemonic}" --signer ss2 "#,
    ));
    assert!(format!("{:?}", result.unwrap_err()).contains("Signer 'ss' is already loaded"));

    let result = sh(&format!("{cli} signer list"));
    let signers = result.get("signers").unwrap();
    assert!(!signers.as_array().unwrap().is_empty());

    let result = sh(&format!("{cli} signer unload --signer ss"));
    let unloaded = result.get("unloaded").unwrap();
    assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "ss");

    let result = sh(&format!("{cli} signer list"));
    let signers = result.get("signers").unwrap();
    assert!(signers.as_array().unwrap().is_empty());

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_signer_external() {
    let (t, _tmp, cli, _params, _server) = setup_cli();

    let name = "ext";
    let fingerprint = "11111111";
    let r = sh(&format!(
        "{cli} signer load-external --fingerprint {fingerprint} --signer {name}"
    ));
    assert_eq!(r.get("name").unwrap().as_str().unwrap(), name);

    // Some actions are not possible with the external signer
    let r = sh_result(&format!("{cli} signer xpub --signer {name} --kind bip84"));
    assert!(format!("{:?}", r.unwrap_err()).contains("Invalid operation for external signer"));
    let r = sh_result(&format!("{cli} signer sign --signer {name} --pset pset"));
    assert!(format!("{:?}", r.unwrap_err()).contains("Invalid operation for external signer"));
    let r = sh_result(&format!(
        "{cli} signer singlesig-desc --signer {name} --descriptor-blinding-key slip77 --kind wpkh"
    ));
    assert!(format!("{:?}", r.unwrap_err()).contains("Invalid operation for external signer"));

    // Load a wallet and see external signer name in the wallet details
    let xpub = "tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87";
    let view_key = "c25deb86fa11e49d651d7eae27c220ef930fbd86ea023eebfa73e54875647963";
    let desc = format!("ct({view_key},elwpkh([{fingerprint}/0h/0h/0h]{xpub}/<0;1>/*))#w2d0h7gl");
    sh(&format!("{cli} wallet load --wallet ss -d {desc}"));

    let r = sh(&format!("{cli} wallet details --wallet ss"));
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 1);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), name);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_wallet_load_unload_list() {
    let (t, _tmp, cli, _params, _server) = setup_cli();

    let result = sh(&format!("{cli} wallet list"));
    let wallets = result.get("wallets").unwrap();
    assert!(wallets.as_array().unwrap().is_empty());

    let desc = "ct(c25deb86fa11e49d651d7eae27c220ef930fbd86ea023eebfa73e54875647963,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#q9cypnmc";
    let result = sh(&format!("{cli} wallet load --wallet custody -d {desc}"));
    assert_eq!(result.get("descriptor").unwrap().as_str().unwrap(), desc);

    let result = sh_result(&format!("{cli} wallet load --wallet custody -d {desc}"));
    assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'custody' is already loaded"));

    let result = sh_result(&format!(
        "{cli} wallet load --wallet differentname -d {desc}"
    ));
    assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'custody' is already loaded"));

    let result = sh(&format!("{cli} wallet list"));
    let wallets = result.get("wallets").unwrap();
    assert!(!wallets.as_array().unwrap().is_empty());

    let result = sh(&format!("{cli} wallet unload --wallet custody"));
    let unloaded = result.get("unloaded").unwrap();
    assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "custody");

    let result = sh(&format!("{cli} wallet list"));
    let wallets = result.get("wallets").unwrap();
    assert!(wallets.as_array().unwrap().is_empty());

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_wallet_details() {
    let (t, _tmp, cli, _params, _server) = setup_cli();

    let r = sh(&format!("{cli} signer generate"));
    let m1 = r.get("mnemonic").unwrap().as_str().unwrap();
    let r = sh(&format!(
        "{cli} signer load-software --mnemonic \"{m1}\" --signer s1"
    ));
    assert_eq!(r.get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!("{cli} signer generate"));
    let m2 = r.get("mnemonic").unwrap().as_str().unwrap();
    let r = sh(&format!(
        "{cli} signer load-software --mnemonic \"{m2}\" --signer s2"
    ));
    assert_eq!(r.get("name").unwrap().as_str().unwrap(), "s2");

    // Single sig wallet
    let r = sh(&format!(
        "{cli} signer singlesig-desc --signer s1 --descriptor-blinding-key slip77 --kind wpkh"
    ));
    let desc_ss = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --wallet ss -d {desc_ss}"));

    let r = sh(&format!(
        "{cli} signer singlesig-desc --signer s1 --descriptor-blinding-key slip77 --kind shwpkh"
    ));
    let desc_sssh = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --wallet sssh -d {desc_sssh}"));

    let r = sh_result(&format!(
        "{cli} signer singlesig-desc -s s1 --descriptor-blinding-key slip77-rand --kind wpkh"
    ));
    let err = "Random slip77 key not supported in singlesig descriptor generation";
    assert!(format!("{:?}", r.unwrap_err()).contains(err));

    // Multi sig wallet
    let r = sh(&format!("{cli} signer xpub --signer s1 --kind bip84"));
    let xpub1 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} signer xpub --signer s2 --kind bip84"));
    let xpub2 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77-rand --kind wsh --threshold 2 --keyorigin-xpub {xpub1} --keyorigin-xpub {xpub2}"));
    let desc_ms = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --wallet ms -d {desc_ms}"));

    let r = sh_result(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77 --kind wsh --threshold 2 --keyorigin-xpub {xpub1} --keyorigin-xpub {xpub2}"));
    let err = "Deterministic slip77 key not supported in multisig descriptor generation";
    assert!(format!("{:?}", r.unwrap_err()).contains(err));

    // Multi sig wallet, same signers
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77-rand --kind wsh --threshold 2 --keyorigin-xpub {xpub1} --keyorigin-xpub {xpub1}"));
    let desc_ms_same_signers = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!(
        "{cli} wallet load --wallet ms_same_signers -d {desc_ms_same_signers}"
    ));

    // Details
    let r = sh(&format!("{cli} wallet details --wallet ss"));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert_eq!(r.get("type").unwrap().as_str().unwrap(), "wpkh");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 1);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!("{cli} wallet details --wallet sssh"));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert_eq!(r.get("type").unwrap().as_str().unwrap(), "sh_wpkh");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 1);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!("{cli} wallet details --wallet ms"));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert_eq!(r.get("type").unwrap().as_str().unwrap(), "wsh_multi_2of2");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 2);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");
    assert_eq!(signers[1].get("name").unwrap().as_str().unwrap(), "s2");

    sh(&format!("{cli} signer unload --signer s2"));
    let r = sh(&format!("{cli} wallet details --wallet ms"));
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 2);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");
    assert!(signers[1].get("name").is_none());

    let r = sh(&format!("{cli} wallet details --wallet ms_same_signers"));
    assert_eq!(
        r.get("warnings").unwrap().as_str().unwrap(),
        "wallet has multiple signers with the same fingerprint"
    );
    assert_eq!(r.get("type").unwrap().as_str().unwrap(), "wsh_multi_2of2");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 2);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");
    assert_eq!(signers[1].get("name").unwrap().as_str().unwrap(), "s1");

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_broadcast() {
    let (t, _tmp, cli, _params, server) = setup_cli();

    let result = sh(&format!("{cli} signer generate"));
    let mnemonic = result.get("mnemonic").unwrap().as_str().unwrap();

    let result = sh(&format!(
        r#"{cli} signer load-software --mnemonic "{mnemonic}" --signer s1 "#
    ));
    assert_eq!(result.get("name").unwrap().as_str().unwrap(), "s1");

    let result = sh(&format!(
        r#"{cli} signer singlesig-desc --signer s1 --descriptor-blinding-key slip77 --kind wpkh"#
    ));
    let desc_generated = result.get("descriptor").unwrap().as_str().unwrap();

    let result = sh(&format!(
        r#"{cli} wallet load --wallet w1 -d {desc_generated}"#
    ));
    assert_eq!(
        result.get("descriptor").unwrap().as_str().unwrap(),
        desc_generated
    );

    fund(&server, &cli, "w1", 1_000_000);

    let regtest_policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let result = sh(&format!("{cli} wallet balance --wallet w1"));
    let balance_obj = result.get("balance").unwrap();
    let policy_obj = balance_obj.get(regtest_policy_asset).unwrap();
    assert_eq!(policy_obj.as_number().unwrap().as_u64().unwrap(), 1_000_000);

    let node_address = server.node_getnewaddress();
    let result = sh(&format!(
        r#"{cli} wallet send --wallet w1 --recipient {node_address}:1000:{regtest_policy_asset}"#
    ));
    let pset = result.get("pset").unwrap().as_str().unwrap();
    let pset_unsigned: PartiallySignedTransaction = pset.parse().unwrap();

    let result = sh(&format!(r#"{cli} signer sign --signer s1 --pset {pset}"#));
    let pset = result.get("pset").unwrap().as_str().unwrap();
    let pset_signed: PartiallySignedTransaction = pset.parse().unwrap();

    assert_ne!(pset_signed, pset_unsigned);

    let result = sh(&format!(
        r#"{cli} wallet broadcast --wallet w1 --pset {pset_signed}"#
    ));
    assert!(result.get("txid").unwrap().as_str().is_some());

    let result = sh(&format!("{cli} wallet balance --wallet w1"));
    let balance_obj = result.get("balance").unwrap();
    let policy_obj = balance_obj.get(regtest_policy_asset).unwrap();
    assert!(policy_obj.as_number().unwrap().as_u64().unwrap() < 1_000_000);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_issue() {
    let (t, _tmp, cli, _params, server) = setup_cli();

    let r = sh(&format!("{cli} signer generate"));
    let mnemonic = r.get("mnemonic").unwrap().as_str().unwrap();

    let r = sh(&format!(
        r#"{cli} signer load-software --mnemonic "{mnemonic}" --signer s1 "#
    ));
    assert_eq!(r.get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!(
        "{cli} signer singlesig-desc --signer s1 --descriptor-blinding-key slip77 --kind wpkh"
    ));
    let desc = r.get("descriptor").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} wallet load --wallet w1 -d {desc}"));
    assert_eq!(r.get("descriptor").unwrap().as_str().unwrap(), desc);

    fund(&server, &cli, "w1", 1_000_000);

    let r = sh(&format!("{cli} asset contract --domain example.com --issuer-pubkey 035d0f7b0207d9cc68870abfef621692bce082084ed3ca0c1ae432dd12d889be01 --name example --ticker EXMP"));
    let contract = serde_json::to_string(&r).unwrap();
    let r = sh(&format!(
        "{cli} wallet issue --wallet w1 --satoshi-asset 1000 --satoshi-token 1 --contract '{contract}'"
    ));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let pset_unsigned: PartiallySignedTransaction = pset.parse().unwrap();

    let r = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert!(r.get("fee").unwrap().as_u64().unwrap() > 0);
    assert!(r.get("reissuances").unwrap().as_array().unwrap().is_empty());
    let issuances = r.get("issuances").unwrap().as_array().unwrap();
    assert_eq!(issuances.len(), 1);
    let issuance = &issuances[0].as_object().unwrap();
    assert_eq!(issuance.get("vin").unwrap().as_u64().unwrap(), 0);
    assert!(!issuance.get("is_confidential").unwrap().as_bool().unwrap());
    let asset = issuance.get("asset").unwrap().as_str().unwrap();
    let token = issuance.get("token").unwrap().as_str().unwrap();
    let asset_sats = issuance.get("asset_satoshi").unwrap().as_u64().unwrap();
    let token_sats = issuance.get("token_satoshi").unwrap().as_u64().unwrap();
    assert_eq!(asset_sats, 1000);
    assert_eq!(token_sats, 1);
    let prev_txid = issuance.get("prev_txid").unwrap().as_str().unwrap();
    let prev_vout = issuance.get("prev_vout").unwrap().as_u64().unwrap();

    let balance = r.get("balance").unwrap().as_object().unwrap();
    // TODO: util to check balance with less unwrap
    assert_eq!(balance.get(asset).unwrap().as_i64().unwrap(), 1000);
    assert_eq!(balance.get(token).unwrap().as_i64().unwrap(), 1);

    let r = sh(&format!(
        "{cli} wallet pset-details --wallet w1 -p {pset} --with-tickers"
    ));
    let balance = r.get("balance").unwrap().as_object().unwrap();
    assert!(balance.get("L-BTC").unwrap().as_i64().unwrap() < 0);

    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let pset_signed: PartiallySignedTransaction = pset.parse().unwrap();

    assert_ne!(pset_signed, pset_unsigned);

    let r = sh(&format!(
        "{cli} wallet broadcast --wallet w1 --pset {pset_signed}"
    ));
    assert!(r.get("txid").unwrap().as_str().is_some());

    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let r = sh(&format!("{cli} wallet balance --wallet w1"));
    let balance = r.get("balance").unwrap().as_object().unwrap();
    assert_eq!(balance.get(asset).unwrap().as_u64().unwrap(), 1000);

    let r = sh(&format!("{cli} wallet balance --wallet w1 --with-tickers"));
    let balance = r.get("balance").unwrap().as_object().unwrap();
    assert!(balance.get("L-BTC").unwrap().as_u64().unwrap() > 0);

    let r = sh(&format!("{cli} asset details --asset {policy_asset}"));
    let name = r.get("name").unwrap().as_str().unwrap();
    assert_eq!(name, "liquid bitcoin");
    assert_eq!(r.get("ticker").unwrap().as_str().unwrap(), "L-BTC");

    let r = sh(&format!("{cli} asset list"));
    let assets = r.get("assets").unwrap().as_array().unwrap();
    assert_eq!(assets.len(), 1);

    let prevout = format!("--prev-txid {prev_txid} --prev-vout {prev_vout}");
    sh(&format!(
        "{cli} asset insert --asset {asset} --contract '{contract}' {prevout}"
    ));

    let result = sh(&format!("{cli} asset list"));
    let assets = result.get("assets").unwrap().as_array().unwrap();
    assert_eq!(assets.len(), 3);

    let r = sh(&format!("{cli} asset details --asset {asset}"));
    let name = r.get("name").unwrap().as_str().unwrap();
    assert_eq!(name, "example");

    let reissuance_token_name = &format!("reissuance token for {name}");
    let r = sh(&format!("{cli} asset details --asset {token}"));
    let name = r.get("name").unwrap().as_str().unwrap();
    assert_eq!(name, reissuance_token_name);

    sh(&format!("{cli} asset remove --asset {token}"));
    let r = sh(&format!("{cli} asset list"));
    let assets = r.get("assets").unwrap().as_array().unwrap();
    assert_eq!(assets.len(), 2);

    let asset_balance_pre = get_balance(&cli, "w1", asset);
    let node_address = server.node_getnewaddress();
    let recipient = format!("--recipient {node_address}:1:{asset}");
    let r = sh(&format!("{cli} wallet send --wallet w1 {recipient}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    // TODO: add PSET introspection verifying there are asset metadata
    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} wallet broadcast --wallet w1 --pset {pset}"));
    let _txid = r.get("txid").unwrap().as_str().unwrap();
    let asset_balance_post = get_balance(&cli, "w1", asset);
    assert_eq!(asset_balance_pre, asset_balance_post + 1);

    let r = sh(&format!(
        "{cli} wallet reissue --wallet w1 --asset {asset} --satoshi-asset 1"
    ));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} wallet broadcast --wallet w1 --pset {pset}"));
    let _txid = r.get("txid").unwrap().as_str().unwrap();
    assert_eq!(asset_balance_post + 1, get_balance(&cli, "w1", asset));

    let recipient = format!("--recipient burn:1:{asset}");
    let r = sh(&format!("{cli} wallet send --wallet w1 {recipient}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} wallet broadcast --wallet w1 --pset {pset}"));
    let _txid = r.get("txid").unwrap().as_str().unwrap();
    assert_eq!(asset_balance_post, get_balance(&cli, "w1", asset));

    let r = sh(&format!("{cli} wallet utxos --wallet w1"));
    let utxos = r.get("utxos").unwrap().as_array().unwrap();
    assert!(!utxos.is_empty());

    let r = sh(&format!("{cli} wallet txs --wallet w1"));
    let txs = r.get("txs").unwrap().as_array().unwrap();
    assert!(!txs.is_empty());

    for tx in txs {
        let balance = tx.get("balance").unwrap().as_object().unwrap();
        assert!(balance.get(policy_asset).is_some());

        if tx.get("height").is_some() {
            assert!(tx.get("timestamp").is_some());
        }

        assert!(tx.get("fee").unwrap().as_u64().unwrap() > 0);
        let types = ["issuance", "reissuance", "burn", "incoming", "outgoing"];
        assert!(types.contains(&tx.get("type").unwrap().as_str().unwrap()));
        // Always received or spent L-BTC
        let url = tx.get("unblinded_url").unwrap().as_str().unwrap();
        assert!(url.contains(policy_asset));
    }

    server.generate(1);

    let r = sh(&format!("{cli} wallet txs --wallet w1 --with-tickers"));
    let txs = r.get("txs").unwrap().as_array().unwrap();
    assert!(!txs.is_empty());

    for tx in txs {
        assert!(tx.get("height").is_some());
        assert!(tx.get("timestamp").is_some());
    }

    let balance = txs[0].get("balance").unwrap().as_object().unwrap();
    assert!(balance.contains_key("L-BTC"));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_jade_emulator() {
    let (t, _tmp, cli, _params, server) = setup_cli();

    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let jade_addr = format!("127.0.0.1:{}", port);

    let result = sh(&format!("{cli} signer jade-id --emulator {jade_addr}"));
    let identifier = result.get("identifier").unwrap().as_str().unwrap();
    assert_eq!(identifier, "e3ebcc79ebfedb4f2ae34406827dc1c5cb48e11f");

    let result = sh(&format!(
        "{cli} signer load-jade --signer emul --id {identifier}  --emulator {jade_addr}"
    ));
    assert!(result.get("id").is_some());
    // Load singlesig wallets
    singlesig_wallet(&cli, "ss-wpkh", "emul", "slip77", "wpkh");
    singlesig_wallet(&cli, "ss-shwpkh", "emul", "slip77", "shwpkh");

    // Use jade in a multisig wallet
    sw_signer(&cli, "sw");
    let signers = &["sw", "emul"];
    multisig_wallet(&cli, "multi", 2, signers, "slip77-rand");
    fund(&server, &cli, "multi", 10_000);
    let addr = address(&cli, "multi");
    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    send(&cli, "multi", &addr, policy_asset, 1_000, signers);

    // Confirm the address on jade
    sh(&format!("{cli} wallet address -w ss-wpkh -s emul"));
    sh(&format!("{cli} wallet address -w ss-shwpkh -s emul"));
    sh(&format!("{cli} wallet address -w multi -s emul"));

    singlesig_wallet(&cli, "ss-sw", "sw", "slip77", "wpkh");
    let r = sh_result(&format!("{cli} wallet address -w ss-sw -s emul"));
    assert!(format!("{:?}", r.unwrap_err()).contains("Signer is not in wallet"));

    let r = sh_result(&format!("{cli} wallet address -w ss-sw -s sw"));
    assert!(format!("{:?}", r.unwrap_err()).contains("Cannot display address with software signer"));

    sh(&format!("{cli} server stop"));
    std::thread::sleep(std::time::Duration::from_millis(100));
    t.join().unwrap();
}

#[test]
fn test_commands() {
    let (t, _tmp, cli, _params, server) = setup_cli();

    let result = sh(&format!("{cli} signer generate"));
    assert!(result.get("mnemonic").is_some());

    let desc = "ct(c25deb86fa11e49d651d7eae27c220ef930fbd86ea023eebfa73e54875647963,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#q9cypnmc";
    let result = sh(&format!("{cli} wallet load --wallet custody -d {desc}"));
    assert_eq!(result.get("descriptor").unwrap().as_str().unwrap(), desc);

    let result = sh_result(&format!("{cli} wallet load --wallet wrong -d wrong"));
    assert!(
        format!("{:?}", result.unwrap_err()).contains("Invalid descriptor: Not a CT Descriptor")
    );

    fund(&server, &cli, "custody", 1_000_000);

    let result = sh(&format!("{cli}  wallet balance --wallet custody"));
    let balance_obj = result.get("balance").unwrap();
    let asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let policy_obj = balance_obj.get(asset).unwrap();
    assert_eq!(policy_obj.as_number().unwrap().as_u64().unwrap(), 1000000);

    let result = sh_result(&format!("{cli}  wallet balance --wallet notexist"));
    assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'notexist' does not exist"));

    let result = sh(&format!("{cli} wallet address --wallet custody"));
    assert_eq!(result.get("address").unwrap().as_str().unwrap(), "el1qqdtwgfchn6rtl8peyw6afhrkpphqlyxls04vlwycez2fz6l7chlhxr8wtvy9s2v34f9sk0e2g058p0dwdp9kj38296xw5ur70");
    assert_eq!(result.get("index").unwrap().as_u64().unwrap(), 1);

    let result = sh(&format!("{cli} wallet address --wallet custody --index 0"));
    assert_eq!(result.get("address").unwrap().as_str().unwrap(), "el1qqg0nthgrrl4jxeapsa40us5d2wv4ps2y63pxwqpf3zk6y69jderdtzfyr95skyuu3t03sh0fvj09f9xut8erjly3ndquhu0ry");
    assert_eq!(result.get("index").unwrap().as_u64().unwrap(), 0);

    let result = sh(&format!("{cli} wallet send --wallet custody --recipient el1qqdtwgfchn6rtl8peyw6afhrkpphqlyxls04vlwycez2fz6l7chlhxr8wtvy9s2v34f9sk0e2g058p0dwdp9kj38296xw5ur70:2:5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225"));
    let pset = result.get("pset").unwrap().as_str().unwrap();
    let _: PartiallySignedTransaction = pset.parse().unwrap();

    let result = sh(&format!("{cli}  wallet unload --wallet custody"));
    let unloaded = result.get("unloaded").unwrap();
    assert_eq!(unloaded.get("descriptor").unwrap().as_str().unwrap(), desc);
    assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "custody");

    let mnemonic = lwk_test_util::TEST_MNEMONIC;
    let result = sh(&format!(
        r#"{cli} signer load-software --mnemonic "{mnemonic}" --signer ss "#
    ));
    assert_eq!(result.get("name").unwrap().as_str().unwrap(), "ss");

    let result = sh(&format!(
        "{cli} signer singlesig-desc --signer ss --descriptor-blinding-key slip77 --kind wpkh"
    ));
    let desc_generated = result.get("descriptor").unwrap().as_str().unwrap();

    let result = sh(&format!(
        "{cli} wallet load --wallet desc_generated -d {desc_generated}"
    ));
    let result = result.get("descriptor").unwrap().as_str().unwrap();
    assert_eq!(result, desc_generated);

    let result = sh(&format!(
        "{cli} wallet address --wallet desc_generated --index 0"
    ));
    assert_eq!(result.get("address").unwrap().as_str().unwrap(), "el1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z0z676mna6kdq");
    assert_eq!(result.get("index").unwrap().as_u64().unwrap(), 0);

    let result = sh(&format!("{cli} signer xpub --signer ss --kind bip84"));
    let keyorigin_xpub = result.get("keyorigin_xpub").unwrap().as_str().unwrap();
    assert_eq!(keyorigin_xpub, "[73c5da0a/84h/1h/0h]tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M");

    let result = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77-rand --kind wsh --threshold 1 --keyorigin-xpub {keyorigin_xpub}"));
    let multisig_desc_generated = result.get("descriptor").unwrap().as_str().unwrap();

    let result = sh(&format!(
        "{cli} wallet load --wallet multi_desc_generated -d {multisig_desc_generated}"
    ));
    let result = result.get("descriptor").unwrap().as_str().unwrap();
    assert_eq!(result, multisig_desc_generated);

    sh(&format!("{cli} server stop"));
    std::thread::sleep(std::time::Duration::from_millis(100));
    t.join().unwrap();
}

#[test]
fn test_multisig() {
    let (t, _tmp, cli, _params, server) = setup_cli();

    let r = sh(&format!("{cli} signer generate"));
    let m1 = r.get("mnemonic").unwrap().as_str().unwrap();
    sh(&format!(
        r#"{cli} signer load-software --mnemonic "{m1}" --signer s1 "#
    ));
    let r = sh(&format!("{cli} signer generate"));
    let m2 = r.get("mnemonic").unwrap().as_str().unwrap();
    sh(&format!(
        r#"{cli} signer load-software --mnemonic "{m2}" --signer s2 "#
    ));

    let r = sh(&format!("{cli} signer xpub --signer s1 --kind bip84"));
    let keyorigin_xpub1 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} signer xpub --signer s2 --kind bip84"));
    let keyorigin_xpub2 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77-rand --kind wsh --threshold 2 --keyorigin-xpub {keyorigin_xpub1} --keyorigin-xpub {keyorigin_xpub2}"));
    let desc = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --wallet multi -d {desc}"));

    fund(&server, &cli, "multi", 1_000_000);

    let node_address = server.node_getnewaddress();
    let satoshi = 1000;
    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let recipient = format!("{node_address}:{satoshi}:{policy_asset}");
    let r = sh(&format!(
        "{cli} wallet send --wallet multi --recipient {recipient}"
    ));
    let pset_u = r.get("pset").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset_u}"));
    let pset_s1 = r.get("pset").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} signer sign --signer s2 --pset {pset_u}"));
    let pset_s2 = r.get("pset").unwrap().as_str().unwrap();

    assert_ne!(pset_u, pset_s1);
    assert_ne!(pset_u, pset_s2);
    assert_ne!(pset_s1, pset_s2);

    let r = sh(&format!(
        "{cli} wallet pset-details --wallet multi -p {pset_u}"
    ));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert!(!r.get("balance").unwrap().as_object().unwrap().is_empty());
    assert!(r.get("fee").unwrap().as_u64().unwrap() > 0);
    assert!(r.get("issuances").unwrap().as_array().unwrap().is_empty());
    assert!(r.get("reissuances").unwrap().as_array().unwrap().is_empty());
    let has_sigs = r.get("has_signatures_from").unwrap().as_array().unwrap();
    assert_eq!(has_sigs.len(), 0);
    let missing_sigs = r
        .get("missing_signatures_from")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(missing_sigs.len(), 2);
    let f = |s: &Value| s.get("name").unwrap().as_str().unwrap().to_string();
    let sigs: HashSet<_> = missing_sigs.iter().map(f).collect();
    assert!(sigs.contains("s1"));
    assert!(sigs.contains("s2"));

    let r = sh(&format!(
        "{cli} wallet pset-details --wallet multi -p {pset_s1}"
    ));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert!(!r.get("balance").unwrap().as_object().unwrap().is_empty());
    assert!(r.get("fee").unwrap().as_u64().unwrap() > 0);
    assert!(r.get("issuances").unwrap().as_array().unwrap().is_empty());
    assert!(r.get("reissuances").unwrap().as_array().unwrap().is_empty());
    let has_sigs = r.get("has_signatures_from").unwrap().as_array().unwrap();
    assert_eq!(has_sigs.len(), 1);
    assert_eq!(has_sigs[0].get("name").unwrap().as_str().unwrap(), "s1");
    let missing_sigs = r
        .get("missing_signatures_from")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(missing_sigs.len(), 1);
    assert_eq!(missing_sigs[0].get("name").unwrap().as_str().unwrap(), "s2");

    let r = sh(&format!(
        "{cli} wallet pset-details --wallet multi -p {pset_s2}"
    ));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert!(!r.get("balance").unwrap().as_object().unwrap().is_empty());
    assert!(r.get("fee").unwrap().as_u64().unwrap() > 0);
    assert!(r.get("issuances").unwrap().as_array().unwrap().is_empty());
    assert!(r.get("reissuances").unwrap().as_array().unwrap().is_empty());
    let has_sigs = r.get("has_signatures_from").unwrap().as_array().unwrap();
    assert_eq!(has_sigs.len(), 1);
    assert_eq!(has_sigs[0].get("name").unwrap().as_str().unwrap(), "s2");
    let missing_sigs = r
        .get("missing_signatures_from")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(missing_sigs.len(), 1);
    assert_eq!(missing_sigs[0].get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!(
        "{cli} wallet combine --wallet multi -p {pset_s1} -p {pset_s2}"
    ));
    let pset_s = r.get("pset").unwrap().as_str().unwrap();

    let r = sh(&format!(
        "{cli} wallet broadcast --wallet multi --pset {pset_s}"
    ));
    let _txid = r.get("txid").unwrap().as_str().unwrap();

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_inconsistent_network() {
    let (_t, _tmp, cli, _params, _server) = setup_cli();
    let cli_addr = cli.split(" -n").next().unwrap();
    let r = sh_result(&format!("{cli_addr} -n testnet wallet list"));
    assert!(format!("{:?}", r.unwrap_err()).contains("Inconsistent network"));
}

#[test]
fn test_schema() {
    let (t, _tmp, cli, _params, _server) = setup_cli();

    for a in WalletSubCommandsEnum::value_variants() {
        let a = a.to_possible_value();
        let cmd = a.map(|e| e.get_name().to_string()).unwrap();
        let result = sh(&format!("{cli} schema request wallet {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {}", cmd);

        let result = sh(&format!("{cli} schema response wallet {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {}", cmd);
    }

    for a in SignerSubCommandsEnum::value_variants() {
        let a = a.to_possible_value();
        let cmd = a.map(|e| e.get_name().to_string()).unwrap();
        let result = sh(&format!("{cli} schema request signer {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {}", cmd);

        let result = sh(&format!("{cli} schema response signer {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {}", cmd);
    }

    for a in AssetSubCommandsEnum::value_variants() {
        let a = a.to_possible_value();
        let cmd = a.map(|e| e.get_name().to_string()).unwrap();
        let result = sh(&format!("{cli} schema request asset {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {}", cmd);

        let result = sh(&format!("{cli} schema response asset {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {}", cmd);
    }

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_elip151() {
    let (t, _tmp, cli, _params, _server) = setup_cli();

    sw_signer(&cli, "s1");
    sw_signer(&cli, "s2");

    let r = sh(&format!(
        "{cli} signer singlesig-desc -s s1 --descriptor-blinding-key elip151 --kind wpkh"
    ));
    let desc_ss = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --wallet ss -d {desc_ss}"));

    let signers = &["s1", "s2"];
    multisig_wallet(&cli, "multi", 2, signers, "elip151");

    // Load a jade
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let addr = format!("127.0.0.1:{}", port);
    let r = sh(&format!("{cli} signer jade-id --emulator {addr}"));
    let id = r.get("identifier").unwrap().as_str().unwrap();
    assert_eq!(id, "e3ebcc79ebfedb4f2ae34406827dc1c5cb48e11f");
    sh(&format!(
        "{cli} signer load-jade --signer emul --id {id}  --emulator {addr}"
    ));

    // Create a elip151 multisig wallet with jade (mj)
    let xpubs = format!(
        "--keyorigin-xpub {} --keyorigin-xpub {}",
        keyorigin(&cli, "s1"),
        keyorigin(&cli, "emul")
    );
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key elip151 --kind wsh --threshold 2 {xpubs}"));
    let d = get_str(&r, "descriptor");
    sh(&format!("{cli} wallet load --wallet mj -d {d}"));

    // Registering the sw wallet works (no-op)
    sh(&format!("{cli} signer register-multisig -s s1 --wallet mj"));
    // Jade fails though because it does not support elip151 keys
    let r = sh_result(&format!(
        "{cli} signer register-multisig -s emul --wallet mj"
    ));
    assert!(format!("{:?}", r.unwrap_err())
        .contains("Jade Error: Only slip77 master blinding key are supported"));

    // Jade does not support elip151 for singlesig too,
    // but since it assumes that the key is slip77 we can do nothing about it.
    let r = sh(&format!(
        "{cli} signer singlesig-desc -s emul --descriptor-blinding-key elip151 --kind wpkh"
    ));
    let desc_ssj = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load -w ssj -d {desc_ssj}"));
    let r = sh_result(&format!("{cli} wallet address -w ssj -s emul"));
    assert!(
        format!("{:?}", r.unwrap_err()).contains("Mismatching addresses between wallet and jade")
    );

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}
