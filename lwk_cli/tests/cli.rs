use std::{
    collections::HashSet,
    fs,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    process::{Child, Command, Stdio},
    str::FromStr,
    thread::JoinHandle,
};

use clap::{Parser, ValueEnum};
use elements::hex::ToHex;
use elements::{encode::serialize, Txid};
use elements::{pset::PartiallySignedTransaction, Address};
use lwk_containers::{testcontainers::clients, JadeEmulator, EMULATOR_PORT};
use serde_json::Value;

use lwk_cli::{
    inner_main, AssetSubCommandsEnum, Cli, ServerSubCommandsEnum, SignerSubCommandsEnum,
    WalletSubCommandsEnum,
};
use lwk_test_util::{TestEnv, TestEnvBuilder};
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
    sh(&format!("{cli} server scan"));
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
    sh_result(command).unwrap()
}

fn sh_err(command: &str) -> String {
    format!("{:?}", sh_result(command).unwrap_err())
}

struct RegistryProc {
    child: Child,
    pub url: String,
}

impl Drop for RegistryProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn setup_cli(
    env: TestEnv,
    with_registry: bool,
) -> (
    JoinHandle<()>,
    TempDir,
    String,
    String,
    TestEnv,
    Option<RegistryProc>,
) {
    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();

    let stderr = if std::env::var_os("RUST_LOG").is_some() {
        Stdio::inherit()
    } else {
        Stdio::null()
    };

    let child = if with_registry {
        let addr = get_available_addr().unwrap();
        let url = format!("127.0.0.1:{}", addr.port());
        let esplora_url = env.esplora_url();
        let child = Command::new("server")
            .args(["--addr", &url])
            .args(["--db-path", &datadir])
            .args(["--esplora-url", &esplora_url])
            .stderr(stderr)
            .spawn()
            .unwrap();
        Some(RegistryProc { child, url })
    } else {
        None
    };

    let registry_url = child
        .as_ref()
        .map(|r| format!("--registry-url http://{}/", r.url))
        .unwrap_or("".to_owned());

    let server_url = format!("--server-url {}", &env.electrum_url());
    let addr = get_available_addr().unwrap();

    let cli = format!("cli --addr {addr} -n regtest");
    let params = format!("--datadir {datadir} {server_url} {registry_url}");

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

    (t, tmp, cli, params, env, child)
}

fn get_str<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).unwrap().as_str().unwrap()
}

fn get_len(v: &Value, key: &str) -> usize {
    v.get(key).unwrap().as_array().unwrap().len()
}

fn get_desc(r: &Value) -> String {
    let desc = get_str(r, "descriptor");
    // The returned descriptor is equivalent but it could be slightly different
    let desc = desc.replace('\'', "h");
    // Changing the descriptor string invalidates the checksum
    remove_checksum(&desc)
}

fn remove_checksum(desc: &str) -> String {
    desc.split('#')
        .collect::<Vec<_>>()
        .first()
        .unwrap()
        .to_string()
}

fn sw_signer(cli: &str, name: &str) {
    let r = sh(&format!("{cli} signer generate"));
    let mnemonic = get_str(&r, "mnemonic");
    sh(&format!(
        "{cli} signer load-software --persist true --mnemonic \"{mnemonic}\" --signer {name}"
    ));
}

fn keyorigin(cli: &str, signer: &str, bip: &str) -> String {
    let r = sh(&format!("{cli} signer xpub --signer {signer} --kind {bip}"));
    get_str(&r, "keyorigin_xpub").to_string()
}

fn multisig_wallet(cli: &str, name: &str, threshold: u32, signers: &[&str], dbk: &str) {
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

fn tx_memo(cli: &str, wallet: &str, txid: &str) -> String {
    get_str(&tx(cli, wallet, txid).unwrap(), "memo").to_string()
}

fn wait_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

fn wait_tx(cli: &str, wallet: &str, txid: &str) {
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

fn address(cli: &str, wallet: &str) -> String {
    let r = sh(&format!("{cli} wallet address --wallet {wallet}"));
    get_str(&r, "address").to_string()
}

fn addr_memo(cli: &str, w: &str, i: u32) -> String {
    let r = sh(&format!("{cli} wallet address --wallet {w} --index {i}"));
    get_str(&r, "memo").to_string()
}

fn asset_ids_from_issuance_pset(cli: &str, wallet: &str, pset: &str) -> (String, String) {
    let r = sh(&format!("{cli} wallet pset-details -w {wallet} -p {pset}"));
    let issuances = r.get("issuances").unwrap().as_array().unwrap();
    let asset = get_str(&issuances[0], "asset").to_string();
    let token = get_str(&issuances[0], "token").to_string();
    (asset, token)
}

fn fund(env: &TestEnv, cli: &str, wallet: &str, sats: u64) -> (Txid, Address) {
    let addr = Address::from_str(&address(cli, wallet)).unwrap();

    let txid = env.elementsd_sendtoaddress(&addr, sats, None);
    // Only 2 blocks are necessary to make coinbase spendable
    env.elementsd_generate(2);
    wait_tx(cli, wallet, &txid.to_string());
    (txid, addr)
}

fn complete(cli: &str, wallet: &str, pset: &str, signers: &[&str]) -> String {
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

fn send(
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

#[test]
fn test_state_regression() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let server_url = format!("--server-url {}", &env.electrum_url());
    let addr = get_available_addr().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();
    let cli = format!("cli --addr {addr} -n regtest");
    let params = format!("--datadir {datadir} {server_url}");

    // copy static state into data dir
    let state = include_str!("./test_data/state.json");
    let mut to = tmp.as_ref().to_path_buf();
    to.push("liquid-regtest");
    fs::create_dir(&to).unwrap();
    to.push("state.json");
    fs::write(to, state).unwrap();

    let t = {
        let cli = cli.clone();

        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(500));

    let r = sh(&format!("{cli} signer list"));
    assert_eq!(get_len(&r, "signers"), 3);

    let r = sh(&format!("{cli} wallet list"));
    assert_eq!(get_len(&r, "wallets"), 1);

    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 3);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_start_stop_persist() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, params, _env, _) = setup_cli(env, false);

    let r = sh(&format!("{cli} signer list"));
    assert_eq!(get_len(&r, "signers"), 0);

    let mnemonic = lwk_test_util::TEST_MNEMONIC;
    sh(&format!(
        r#"{cli} signer load-software --persist true --mnemonic "{mnemonic}" --signer s1"#
    ));
    let result = sh(&format!("{cli} signer generate"));
    let different_mnemonic = result.get("mnemonic").unwrap().as_str().unwrap();
    sh(&format!(
        r#"{cli} signer load-software --persist true --mnemonic "{different_mnemonic}" --signer s2"#,
    ));
    sh(&format!(r#"{cli} signer unload --signer s2"#)); // Verify unloads are handled

    sh(&format!(
        "{cli} signer load-external --fingerprint 11111111 --signer s2"
    ));
    sh(&format!(
        "{cli} signer load-jade --id 2111111111111111111111111111111111111112 --signer s3"
    ));
    let r = sh(&format!("{cli} signer details -s s1"));
    assert_eq!(get_str(&r, "mnemonic"), mnemonic);
    assert_eq!(get_str(&r, "type"), "software");
    let r = sh(&format!("{cli} signer details -s s2"));
    assert!(r.get("mnemonic").is_none());
    assert_eq!(get_str(&r, "type"), "external");
    let r = sh(&format!("{cli} signer details -s s3"));
    assert!(r.get("mnemonic").is_none());
    assert_eq!(get_str(&r, "type"), "jade-id");

    let desc = "ct(c25deb86fa11e49d651d7eae27c220ef930fbd86ea023eebfa73e54875647963,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#q9cypnmc";
    sh(&format!("{cli} wallet load --wallet custody -d {desc}"));
    sh(&format!(r#"{cli} wallet unload --wallet custody"#)); // Verify unloads are handled
    sh(&format!("{cli} wallet load --wallet custody -d {desc}"));

    let contract = "{\"entity\":{\"domain\":\"tether.to\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Tether USD\",\"precision\":8,\"ticker\":\"USDt\",\"version\":0}";
    let asset = "ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2";
    let tx = include_str!("../../lwk_wollet/tests/data/usdt-issuance-tx.hex");
    sh(&format!(
        "{cli} asset insert --asset {asset} --contract '{contract}' --issuance-tx {tx}"
    ));

    let err = sh_err(&format!("{cli} asset from-registry --asset {asset}"));
    assert!(err.contains("already inserted"));

    let expected_signers = sh(&format!("{cli} signer list"));
    assert_eq!(get_len(&expected_signers, "signers"), 3);

    let expected_wallets = sh(&format!("{cli} wallet list"));
    assert_eq!(get_len(&expected_wallets, "wallets"), 1);

    let expected_assets = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&expected_assets, "assets"), 3);

    // Add another signer that is not persisted
    let r = sh(&format!("{cli} signer generate"));
    let m = get_str(&r, "mnemonic");
    sh(&format!(
        "{cli} signer load-software --persist false --mnemonic '{m}' --signer s4"
    ));
    let r = sh(&format!("{cli} signer list"));
    assert_eq!(get_len(&r, "signers"), 4);

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
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env, _) = setup_cli(env, false);

    let r = sh(&format!("{cli} signer list"));
    assert_eq!(get_len(&r, "signers"), 0);

    sw_signer(&cli, "s1");
    let r = sh(&format!("{cli} signer details -s s1"));
    let m1 = get_str(&r, "mnemonic");
    let m2 = lwk_test_util::TEST_MNEMONIC;

    assert_ne!(m1, m2);
    // Same name, different mnemonic
    let err = sh_err(&format!(
        "{cli} signer load-software --persist true --mnemonic '{m2}' --signer s1"
    ));
    assert!(err.contains("Signer 's1' is already loaded"));

    // Same mnemonic, different name
    let err = sh_err(&format!(
        "{cli} signer load-software --persist true --mnemonic '{m1}' --signer s2"
    ));
    assert!(err.contains("Signer 's1' is already loaded"));

    let r = sh(&format!("{cli} signer list"));
    assert_eq!(get_len(&r, "signers"), 1);

    let r = sh(&format!("{cli} signer unload --signer s1"));
    assert_eq!(get_str(r.get("unloaded").unwrap(), "name"), "s1");

    let r = sh(&format!("{cli} signer list"));
    assert_eq!(get_len(&r, "signers"), 0);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_signer_external() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env, _) = setup_cli(env, false);

    let name = "ext";
    let fingerprint = "11111111";
    let r = sh(&format!(
        "{cli} signer load-external --fingerprint {fingerprint} --signer {name}"
    ));
    assert_eq!(r.get("name").unwrap().as_str().unwrap(), name);

    // Some actions are not possible with the external signer
    let err = sh_err(&format!("{cli} signer xpub --signer {name} --kind bip84"));
    assert!(err.contains("Invalid operation for external signer"));
    let err = sh_err(&format!("{cli} signer sign --signer {name} --pset pset"));
    assert!(err.contains("Invalid operation for external signer"));
    let err = sh_err(&format!(
        "{cli} signer singlesig-desc --signer {name} --descriptor-blinding-key slip77 --kind wpkh"
    ));
    assert!(err.contains("Invalid operation for external signer"));

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
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env, _) = setup_cli(env, false);

    let r = sh(&format!("{cli} wallet list"));
    assert_eq!(get_len(&r, "wallets"), 0);

    let desc = "ct(c25deb86fa11e49d651d7eae27c220ef930fbd86ea023eebfa73e54875647963,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#q9cypnmc";
    let result = sh(&format!("{cli} wallet load --wallet custody -d {desc}"));
    assert_eq!(result.get("descriptor").unwrap().as_str().unwrap(), desc);

    let err = sh_err(&format!("{cli} wallet load --wallet custody -d {desc}"));
    assert!(err.contains("Wallet 'custody' is already loaded"));

    let err = sh_err(&format!(
        "{cli} wallet load --wallet differentname -d {desc}"
    ));
    assert!(err.contains("Wallet 'custody' is already loaded"));

    let r = sh(&format!("{cli} wallet list"));
    assert_eq!(get_len(&r, "wallets"), 1);

    let result = sh(&format!("{cli} wallet unload --wallet custody"));
    let unloaded = result.get("unloaded").unwrap();
    assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "custody");

    let r = sh(&format!("{cli} wallet list"));
    assert_eq!(get_len(&r, "wallets"), 0);

    let desc_mainnet = "ct(1111111111111111111111111111111111111111111111111111111111111111,elwpkh(xpub661MyMwAqRbcH4oCG7tpubMCYWM3pHRZbhBQgi7uVZGcu1EuuomWqwB5gGHXk4VykarKGVA2jKtT4esCXspWW45mzwAzZEsi3U5j94gCKXc/*))";
    let err = sh_err(&format!(
        "{cli} wallet load --wallet main -d {desc_mainnet}"
    ));
    assert!(err.contains("Descriptor is for the wrong network"));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_wallet_memos() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, params, env, _) = setup_cli(env, false);

    // Create 2 wallets
    sw_signer(&cli, "s1");
    sw_signer(&cli, "s2");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    singlesig_wallet(&cli, "w2", "s2", "slip77", "wpkh");

    // Fund w1
    let _ = fund(&env, &cli, "w1", 1_000_000);

    // Send from w1 to w2
    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let w2_addr = address(&cli, "w2");
    let txid = send(&cli, "w1", &w2_addr, policy_asset, 1_000, &["s1"]);

    let r = sh(&format!("{cli} wallet address --wallet w1"));
    let w1_addr = get_str(&r, "address").to_string();
    let index = r.get("index").unwrap().as_u64().unwrap() as u32;

    // Memo are empty for both wallets
    assert_eq!(tx_memo(&cli, "w1", &txid), "");
    assert_eq!(tx_memo(&cli, "w2", &txid), "");
    assert_eq!(addr_memo(&cli, "w1", index), "");

    // Set memo for w1
    let memo1 = "MEMO1";
    sh(&format!(
        "{cli} wallet set-tx-memo -w w1 --txid {txid} --memo {memo1}"
    ));
    assert_eq!(tx_memo(&cli, "w1", &txid), memo1);
    assert_eq!(tx_memo(&cli, "w2", &txid), "");

    sh(&format!(
        "{cli} wallet set-addr-memo -w w1 --address {w1_addr} --memo {memo1}"
    ));
    assert_eq!(addr_memo(&cli, "w1", index), memo1);

    // Set another memo for w2
    let memo2 = "MEMO2";
    sh(&format!(
        "{cli} wallet set-tx-memo -w w2 --txid {txid} --memo {memo2}"
    ));
    assert_eq!(tx_memo(&cli, "w1", &txid), memo1);
    assert_eq!(tx_memo(&cli, "w2", &txid), memo2);

    // Unload and load wallet, memo is removed
    sh(&format!("{cli} wallet unload --wallet w1"));
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    assert_eq!(tx_memo(&cli, "w1", &txid), "");
    assert_eq!(tx_memo(&cli, "w2", &txid), memo2);
    assert_eq!(addr_memo(&cli, "w1", index), "");

    // Remove memo
    sh(&format!(
        "{cli} wallet set-tx-memo -w w2 --txid {txid} --memo ''"
    ));
    assert_eq!(tx_memo(&cli, "w1", &txid), "");
    assert_eq!(tx_memo(&cli, "w2", &txid), "");

    // It's possible to set a memo for any address (w1_addr does not belog to w2)
    sh(&format!(
        "{cli} wallet set-addr-memo -w w2 --address {w1_addr} --memo {memo1}"
    ));
    // But you can't get it

    // Set memos
    sh(&format!(
        "{cli} wallet set-tx-memo -w w1 --txid {txid} --memo {memo1}"
    ));
    assert_eq!(tx_memo(&cli, "w1", &txid), memo1);

    sh(&format!(
        "{cli} wallet set-addr-memo -w w1 --address {w1_addr} --memo {memo1}"
    ));
    assert_eq!(addr_memo(&cli, "w1", index), memo1);

    // And unload w2 to trigger a global persistence
    sh(&format!("{cli} wallet unload --wallet w2"));

    // Stop and restart to check persistence
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

    assert_eq!(tx_memo(&cli, "w1", &txid), memo1);
    assert_eq!(addr_memo(&cli, "w1", index), memo1);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_liquidex() {
    // Test liquidex swap
    // w1 sell asset issued
    // w2 pay with policy asset

    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";

    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    // Create 2 wallets
    sw_signer(&cli, "s1");
    sw_signer(&cli, "s2");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    singlesig_wallet(&cli, "w2", "s2", "slip77", "wpkh");

    let _ = fund(&env, &cli, "w1", 1_000_000);
    let _ = fund(&env, &cli, "w2", 1_000_000);

    let r = sh(&format!("{cli} asset contract --domain example.com --issuer-pubkey 035d0f7b0207d9cc68870abfef621692bce082084ed3ca0c1ae432dd12d889be01 --name example --ticker EXMP"));
    let contract = serde_json::to_string(&r).unwrap();
    let r = sh(&format!(
        "{cli} wallet issue --wallet w1 --satoshi-asset 1000 --satoshi-token 0 --contract '{contract}'"
    ));
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);

    let result = sh(&format!("{cli} wallet utxos --wallet w1"));
    let utxos = result.get("utxos").unwrap().as_array().unwrap();
    let asset_utxo = utxos
        .iter()
        .find(|u| u.get("asset").unwrap().as_str().unwrap() != policy_asset)
        .unwrap();
    let issued_asset_id = asset_utxo.get("asset").unwrap().as_str().unwrap();
    let txid = asset_utxo.get("txid").unwrap().as_str().unwrap();
    let vout = asset_utxo.get("vout").unwrap().as_u64().unwrap();
    let value = asset_utxo.get("value").unwrap().as_u64().unwrap();

    let result = sh(&format!(
        "{cli} liquidex make --wallet w1 --txid {txid} --vout {vout} --asset {policy_asset} --satoshi {value}"
    ));
    let pset = get_str(&result, "pset");
    let pset_unsigned: PartiallySignedTransaction = pset.parse().unwrap();

    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let pset_signed: PartiallySignedTransaction = pset.parse().unwrap();
    assert_ne!(pset_signed, pset_unsigned);

    let r = sh(&format!("{cli} liquidex to-proposal --pset {pset}"));
    let json = &r.get("proposal").unwrap();
    let proposal = serde_json::to_string(json).unwrap();

    let result = sh(&format!(
        "{cli} liquidex take --wallet w2 --proposal '{proposal}'"
    ));
    let pset = get_str(&result, "pset");

    let result = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    println!("result w1: {result:?}"); // TODO: check

    //let result = sh(&format!("{cli} wallet pset-details --wallet w2 -p {pset}"));
    //println!("result w2: {:?}", result);

    let result = sh(&format!("{cli} wallet balance --wallet w1"));
    let balance = result.get("balance").unwrap().as_object().unwrap();
    assert_eq!(
        balance.get(issued_asset_id).unwrap().as_i64().unwrap(),
        1000
    );

    complete(&cli, "w2", pset, &["s2"]);

    let result = sh(&format!("{cli} wallet balance --wallet w2"));
    let balance = result.get("balance").unwrap().as_object().unwrap();
    assert_eq!(
        balance.get(issued_asset_id).unwrap().as_i64().unwrap(),
        1000
    );

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_wallet_details() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env, _) = setup_cli(env, false);

    sw_signer(&cli, "s1");
    sw_signer(&cli, "s2");

    // Single sig wallet
    let r = sh(&format!(
        "{cli} signer singlesig-desc --signer s1 --descriptor-blinding-key slip77 --kind wpkh"
    ));
    let desc_ss = get_str(&r, "descriptor");
    sh(&format!("{cli} wallet load --wallet ss -d {desc_ss}"));
    assert!(desc_ss.contains(&keyorigin(&cli, "s1", "bip84")));

    let r = sh(&format!(
        "{cli} signer singlesig-desc --signer s1 --descriptor-blinding-key slip77 --kind shwpkh"
    ));
    let desc_sssh = get_str(&r, "descriptor");
    sh(&format!("{cli} wallet load --wallet sssh -d {desc_sssh}"));
    assert!(desc_sssh.contains(&keyorigin(&cli, "s1", "bip49")));

    let err = sh_err(&format!(
        "{cli} signer singlesig-desc -s s1 --descriptor-blinding-key slip77-rand --kind wpkh"
    ));
    let exp_err = "Random slip77 key not supported in singlesig descriptor generation";
    assert!(err.contains(exp_err));

    // Multi sig wallet
    let r = sh(&format!("{cli} signer xpub --signer s1 --kind bip87"));
    let xpub1 = get_str(&r, "keyorigin_xpub");
    let r = sh(&format!("{cli} signer xpub --signer s2 --kind bip87"));
    let xpub2 = get_str(&r, "keyorigin_xpub");
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77-rand --kind wsh --threshold 2 --keyorigin-xpub {xpub1} --keyorigin-xpub {xpub2}"));
    let desc_ms = get_str(&r, "descriptor");
    sh(&format!("{cli} wallet load --wallet ms -d {desc_ms}"));

    let err = sh_err(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77 --kind wsh --threshold 2 --keyorigin-xpub {xpub1} --keyorigin-xpub {xpub2}"));
    let exp_err = "Deterministic slip77 key not supported in multisig descriptor generation";
    assert!(err.contains(exp_err));

    // Multi sig wallet, same signers
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77-rand --kind wsh --threshold 2 --keyorigin-xpub {xpub1} --keyorigin-xpub {xpub1}"));
    let desc_ms_same_signers = get_str(&r, "descriptor");
    sh(&format!(
        "{cli} wallet load --wallet ms_same_signers -d {desc_ms_same_signers}"
    ));

    // Details
    let r = sh(&format!("{cli} wallet details --wallet ss"));
    assert_eq!(get_desc(&r), remove_checksum(desc_ss));
    assert!(get_str(&r, "warnings").is_empty());
    assert_eq!(get_str(&r, "type"), "wpkh");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 1);
    assert_eq!(get_str(&signers[0], "name"), "s1");

    let r = sh(&format!("{cli} wallet details --wallet sssh"));
    assert_eq!(get_desc(&r), remove_checksum(desc_sssh));
    assert!(get_str(&r, "warnings").is_empty());
    assert_eq!(get_str(&r, "type"), "sh_wpkh");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 1);
    assert_eq!(get_str(&signers[0], "name"), "s1");

    let r = sh(&format!("{cli} wallet details --wallet ms"));
    assert_eq!(get_desc(&r), remove_checksum(desc_ms));
    assert!(get_str(&r, "warnings").is_empty());
    assert_eq!(get_str(&r, "type"), "wsh_multi_2of2");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 2);
    assert_eq!(get_str(&signers[0], "name"), "s1");
    assert_eq!(get_str(&signers[1], "name"), "s2");

    sh(&format!("{cli} signer unload --signer s2"));
    let r = sh(&format!("{cli} wallet details --wallet ms"));
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 2);
    assert_eq!(get_str(&signers[0], "name"), "s1");
    assert!(signers[1].get("name").is_none());

    let r = sh(&format!("{cli} wallet details --wallet ms_same_signers"));
    assert_eq!(
        get_str(&r, "warnings"),
        "wallet has multiple signers with the same fingerprint"
    );
    assert_eq!(r.get("type").unwrap().as_str().unwrap(), "wsh_multi_2of2");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 2);
    assert_eq!(get_str(&signers[0], "name"), "s1");
    assert_eq!(get_str(&signers[1], "name"), "s1");

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_broadcast() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    sw_signer(&cli, "s1");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w1", 1_000_000);

    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    assert_eq!(1_000_000, get_balance(&cli, "w1", policy_asset));
    let addr = env.elementsd_getnewaddress().to_string();
    send(&cli, "w1", &addr, policy_asset, 1000, &["s1"]);
    assert!(1_000_000 > get_balance(&cli, "w1", policy_asset));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_issue() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    sw_signer(&cli, "s1");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w1", 1_000_000);

    let r = sh(&format!("{cli} asset contract --domain example.com --issuer-pubkey 035d0f7b0207d9cc68870abfef621692bce082084ed3ca0c1ae432dd12d889be01 --name example --ticker EXMP"));
    let contract = serde_json::to_string(&r).unwrap();
    let r = sh(&format!(
        "{cli} wallet issue --wallet w1 --satoshi-asset 1000 --satoshi-token 1 --contract '{contract}'"
    ));
    let pset = get_str(&r, "pset");
    let pset_unsigned: PartiallySignedTransaction = pset.parse().unwrap();

    let r = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    assert!(get_str(&r, "warnings").is_empty());
    assert!(r.get("fee").unwrap().as_u64().unwrap() > 0);
    assert_eq!(get_len(&r, "reissuances"), 0);
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
    let issuance_txid = get_str(&r, "txid");
    sh(&format!("{cli} server scan"));

    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    assert_eq!(get_balance(&cli, "w1", asset), 1000);

    let r = sh(&format!("{cli} wallet balance --wallet w1 --with-tickers"));
    let balance = r.get("balance").unwrap().as_object().unwrap();
    assert!(balance.get("L-BTC").unwrap().as_u64().unwrap() > 0);

    let r = sh(&format!("{cli} asset details --asset {policy_asset}"));
    assert_eq!(get_str(&r, "name"), "liquid bitcoin");
    assert_eq!(get_str(&r, "ticker"), "L-BTC");

    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 1);

    let r = sh(&format!("{cli} wallet tx -w w1 -t {issuance_txid}"));
    let tx = get_str(&r, "tx");
    sh(&format!(
        "{cli} asset insert --asset {asset} --contract '{contract}' --issuance-tx {tx}"
    ));

    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 3);

    let r = sh(&format!("{cli} asset details --asset {asset}"));
    let name = get_str(&r, "name");
    assert_eq!(name, "example");

    let reissuance_token_name = &format!("reissuance token for {name}");
    let r = sh(&format!("{cli} asset details --asset {token}"));
    assert_eq!(get_str(&r, "name"), reissuance_token_name);

    sh(&format!("{cli} asset remove --asset {token}"));
    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 2);

    let asset_balance_pre = get_balance(&cli, "w1", asset);
    let node_address = env.elementsd_getnewaddress();
    let recipient = format!("--recipient {node_address}:1:{asset}");
    let r = sh(&format!("{cli} wallet send --wallet w1 {recipient}"));
    // TODO: add PSET introspection verifying there are asset metadata
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);
    let asset_balance_post = get_balance(&cli, "w1", asset);
    assert_eq!(asset_balance_pre, asset_balance_post + 1);

    let r = sh(&format!(
        "{cli} wallet reissue --wallet w1 --asset {asset} --satoshi-asset 1"
    ));
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);
    assert_eq!(asset_balance_post + 1, get_balance(&cli, "w1", asset));

    let recipient = format!("--recipient burn:1:{asset}");
    let r = sh(&format!("{cli} wallet send --wallet w1 {recipient}"));
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);
    assert_eq!(asset_balance_post, get_balance(&cli, "w1", asset));

    let r = sh(&format!(
        "{cli} wallet burn -w w1 --asset {asset} --satoshi-asset 1"
    ));
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);
    assert_eq!(asset_balance_post - 1, get_balance(&cli, "w1", asset));

    let r = sh(&format!("{cli} wallet utxos --wallet w1"));
    assert!(get_len(&r, "utxos") >= 3);

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

    env.elementsd_generate(1);
    sh(&format!("{cli} server scan"));

    let r = sh(&format!("{cli} wallet txs --wallet w1 --with-tickers"));
    let txs = r.get("txs").unwrap().as_array().unwrap();
    assert!(!txs.is_empty());

    for tx in txs {
        assert!(tx.get("height").is_some());
        assert!(tx.get("timestamp").is_some());
    }

    let balance = txs[0].get("balance").unwrap().as_object().unwrap();
    assert!(balance.contains_key("L-BTC"));

    // Move the reissuance token to another wallet and perform an "external" reissuance
    sw_signer(&cli, "s2");
    singlesig_wallet(&cli, "w2", "s2", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w2", 1_000_000);
    let w2_addr = address(&cli, "w2");
    let txid = send(&cli, "w1", &w2_addr, token, 1, &["s1"]);
    wait_tx(&cli, "w2", &txid);
    let r = sh(&format!(
        "{cli} wallet reissue --wallet w2 --asset {asset} --satoshi-asset 1"
    ));
    complete(&cli, "w2", get_str(&r, "pset"), &["s2"]);
    assert_eq!(1, get_balance(&cli, "w2", asset));

    // Reissue from wallet w1 without token fails with InsufficientFunds
    let err = sh_err(&format!(
        "{cli} wallet reissue --wallet w1 --asset {asset} --satoshi-asset 1"
    ));
    let expected = format!("Insufficient funds: missing 1 units for reissuance token {token}");
    assert!(err.contains(&expected));

    // Removing the asset will cause the "external" reissuance to fail
    sh(&format!("{cli} asset remove --asset {asset}"));
    let err = sh_err(&format!(
        "{cli} wallet reissue --wallet w2 --asset {asset} --satoshi-asset 1"
    ));
    assert!(err.contains("Missing issuance"));

    let err = sh_err(&format!("{cli} wallet tx -w w2 -t {issuance_txid}"));
    assert!(err.contains("was not found in wallet 'w2'"));

    // w2 can get the tx from the explorer
    sh(&format!("{cli} wallet tx -w w2 -t {issuance_txid} --fetch"));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_jade_emulator() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let jade_addr = format!("127.0.0.1:{port}");

    let result = sh(&format!("{cli} signer jade-id --emulator {jade_addr}"));
    let identifier = result.get("identifier").unwrap().as_str().unwrap();
    assert_eq!(identifier, "e3ebcc79ebfedb4f2ae34406827dc1c5cb48e11f");

    sh(&format!(
        "{cli} signer load-jade --signer emul --id {identifier}  --emulator {jade_addr}"
    ));
    let r = sh(&format!("{cli} signer details -s emul"));
    assert!(r.get("id").is_some());
    assert!(r.get("mnemonic").is_none());
    assert_eq!(get_str(&r, "type"), "jade");
    // Load singlesig wallets
    singlesig_wallet(&cli, "ss-wpkh", "emul", "slip77", "wpkh");
    singlesig_wallet(&cli, "ss-shwpkh", "emul", "slip77", "shwpkh");

    // Use jade in a multisig wallet
    sw_signer(&cli, "sw");
    let signers = &["sw", "emul"];
    multisig_wallet(&cli, "multi", 2, signers, "slip77-rand");
    let _ = fund(&env, &cli, "multi", 10_000);
    let addr = address(&cli, "multi");
    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    send(&cli, "multi", &addr, policy_asset, 1_000, signers);

    // Confirm the address on jade
    sh(&format!("{cli} wallet address -w ss-wpkh -s emul"));
    sh(&format!("{cli} wallet address -w ss-shwpkh -s emul"));
    sh(&format!("{cli} wallet address -w multi -s emul"));

    singlesig_wallet(&cli, "ss-sw", "sw", "slip77", "wpkh");
    let err = sh_err(&format!("{cli} wallet address -w ss-sw -s emul"));
    assert!(err.contains("Signer is not in wallet"));

    let err = sh_err(&format!("{cli} wallet address -w ss-sw -s sw"));
    assert!(err.contains("Cannot display address with software signer"));

    sh(&format!("{cli} server stop"));
    std::thread::sleep(std::time::Duration::from_millis(100));
    t.join().unwrap();
}

#[test]
fn test_commands() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    let result = sh(&format!("{cli} signer generate"));
    assert!(result.get("mnemonic").is_some());

    let desc = "ct(c25deb86fa11e49d651d7eae27c220ef930fbd86ea023eebfa73e54875647963,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#q9cypnmc";
    let result = sh(&format!("{cli} wallet load --wallet custody -d {desc}"));
    assert_eq!(result.get("descriptor").unwrap().as_str().unwrap(), desc);

    let err = sh_err(&format!("{cli} wallet load --wallet wrong -d wrong"));
    assert!(err.contains("Invalid descriptor: Not a CT Descriptor"));

    let _ = fund(&env, &cli, "custody", 1_000_000);

    let result = sh(&format!("{cli}  wallet balance --wallet custody"));
    let balance_obj = result.get("balance").unwrap();
    let asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let policy_obj = balance_obj.get(asset).unwrap();
    assert_eq!(policy_obj.as_number().unwrap().as_u64().unwrap(), 1000000);

    let err = sh_err(&format!("{cli}  wallet balance --wallet notexist"));
    assert!(err.contains("Wallet 'notexist' does not exist"));

    let r = sh(&format!("{cli} wallet address --wallet custody"));
    assert_eq!(get_str(&r, "address"), "el1qqdtwgfchn6rtl8peyw6afhrkpphqlyxls04vlwycez2fz6l7chlhxr8wtvy9s2v34f9sk0e2g058p0dwdp9kj38296xw5ur70");
    assert_eq!(r.get("index").unwrap().as_u64().unwrap(), 1);

    let r = sh(&format!("{cli} wallet address --wallet custody --index 0"));
    assert_eq!(get_str(&r, "address"), "el1qqg0nthgrrl4jxeapsa40us5d2wv4ps2y63pxwqpf3zk6y69jderdtzfyr95skyuu3t03sh0fvj09f9xut8erjly3ndquhu0ry");
    assert_eq!(r.get("index").unwrap().as_u64().unwrap(), 0);

    let cli_addr = format!("{cli} wallet address --wallet custody");
    let r = sh(&format!("{cli_addr} --with-text-qr"));
    assert!(get_str(&r, "text_qr").contains('█'));
    assert!(r.get("uri_qr").is_none());

    let r = sh(&format!("{cli_addr} --with-uri-qr 1"));
    assert!(r.get("text_qr").is_none());
    assert!(get_str(&r, "uri_qr").contains("data:image/bmp;base64"));

    let r = sh(&format!("{cli_addr} --with-uri-qr 1 --with-text-qr"));
    assert!(get_str(&r, "text_qr").contains('█'));
    assert!(get_str(&r, "uri_qr").contains("data:image/bmp;base64"));

    let result = sh(&format!("{cli} wallet send --wallet custody --recipient el1qqdtwgfchn6rtl8peyw6afhrkpphqlyxls04vlwycez2fz6l7chlhxr8wtvy9s2v34f9sk0e2g058p0dwdp9kj38296xw5ur70:2:5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225"));
    let pset = result.get("pset").unwrap().as_str().unwrap();
    let _: PartiallySignedTransaction = pset.parse().unwrap();

    let result = sh(&format!("{cli}  wallet unload --wallet custody"));
    let unloaded = result.get("unloaded").unwrap();
    assert_eq!(unloaded.get("descriptor").unwrap().as_str().unwrap(), desc);
    assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "custody");

    let mnemonic = lwk_test_util::TEST_MNEMONIC;
    let result = sh(&format!(
        r#"{cli} signer load-software --persist true --mnemonic "{mnemonic}" --signer ss "#
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
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    sw_signer(&cli, "s1");
    sw_signer(&cli, "s2");

    let r = sh(&format!("{cli} signer xpub --signer s1 --kind bip87"));
    let keyorigin_xpub1 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} signer xpub --signer s2 --kind bip87"));
    let keyorigin_xpub2 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77-rand --kind wsh --threshold 2 --keyorigin-xpub {keyorigin_xpub1} --keyorigin-xpub {keyorigin_xpub2}"));
    let desc = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --wallet multi -d {desc}"));

    let _ = fund(&env, &cli, "multi", 1_000_000);

    let node_address = env.elementsd_getnewaddress();
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
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (_t, _tmp, cli, _params, _env, _) = setup_cli(env, false);
    let cli_addr = cli.split(" -n").next().unwrap();
    let err = sh_err(&format!("{cli_addr} -n testnet wallet list"));
    assert!(err.contains("Inconsistent network"));
}

#[test]
fn test_schema() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env, _) = setup_cli(env, false);

    for a in ServerSubCommandsEnum::value_variants() {
        let a = a.to_possible_value();
        let cmd = a.map(|e| e.get_name().to_string()).unwrap();
        let result = sh(&format!("{cli} schema request server {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {cmd}");

        let result = sh(&format!("{cli} schema response server {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {cmd}");
    }

    for a in WalletSubCommandsEnum::value_variants() {
        let a = a.to_possible_value();
        let cmd = a.map(|e| e.get_name().to_string()).unwrap();
        let result = sh(&format!("{cli} schema request wallet {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {cmd}");

        let result = sh(&format!("{cli} schema response wallet {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {cmd}");
    }

    for a in SignerSubCommandsEnum::value_variants() {
        let a = a.to_possible_value();
        let cmd = a.map(|e| e.get_name().to_string()).unwrap();
        let result = sh(&format!("{cli} schema request signer {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {cmd}");

        let result = sh(&format!("{cli} schema response signer {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {cmd}");
    }

    for a in AssetSubCommandsEnum::value_variants() {
        let a = a.to_possible_value();
        let cmd = a.map(|e| e.get_name().to_string()).unwrap();
        let result = sh(&format!("{cli} schema request asset {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {cmd}");

        let result = sh(&format!("{cli} schema response asset {cmd}"));
        assert!(result.get("$schema").is_some(), "failed for {cmd}");
    }

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[cfg_attr(
    not(feature = "registry"),
    ignore = "require registry `server` executable in path"
)]
#[test]
fn test_registry_publish() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _registry) = setup_cli(env, true);

    sw_signer(&cli, "s1");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w1", 1_000_000);

    let r = sh(&format!("{cli} asset contract --domain example.com --issuer-pubkey 035d0f7b0207d9cc68870abfef621692bce082084ed3ca0c1ae432dd12d889be01 --name example --ticker EXMP"));
    let contract = serde_json::to_string(&r).unwrap();
    let r = sh(&format!(
        "{cli} wallet issue --wallet w1 --satoshi-asset 1000 --satoshi-token 1 --contract '{contract}'"
    ));
    let pset = get_str(&r, "pset");

    let r = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    let issuances = r.get("issuances").unwrap().as_array().unwrap();
    let issuance = &issuances[0].as_object().unwrap();
    let asset = issuance.get("asset").unwrap().as_str().unwrap();
    let token = issuance.get("token").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let pset_signed: PartiallySignedTransaction = pset.parse().unwrap();

    sh(&format!(
        "{cli} wallet broadcast --wallet w1 --pset {pset_signed}"
    ));

    env.elementsd_generate(2);
    wait_ms(6_000); // otherwise registry may find the issuance tx unconfirmed, wait_tx is not enough

    sh(&format!("{cli} server scan"));

    let tx = serialize(&pset_signed.extract_tx().unwrap()).to_hex();
    sh(&format!(
        "{cli} asset insert --asset {asset} --contract '{contract}' --issuance-tx {tx}"
    ));
    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 3);

    sh(&format!("{cli} asset publish --asset {asset}"));

    sh(&format!("{cli} asset remove --asset {asset}"));

    sh(&format!("{cli} asset remove --asset {token}"));

    sh(&format!("{cli} asset list"));

    sh(&format!("{cli} asset from-registry --asset {asset}"));

    sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 3);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_elip151() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env, _) = setup_cli(env, false);

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
    let addr = format!("127.0.0.1:{port}");
    let r = sh(&format!("{cli} signer jade-id --emulator {addr}"));
    let id = r.get("identifier").unwrap().as_str().unwrap();
    assert_eq!(id, "e3ebcc79ebfedb4f2ae34406827dc1c5cb48e11f");
    sh(&format!(
        "{cli} signer load-jade --signer emul --id {id}  --emulator {addr}"
    ));

    // Create a elip151 multisig wallet with jade (mj)
    let xpubs = format!(
        "--keyorigin-xpub {} --keyorigin-xpub {}",
        keyorigin(&cli, "s1", "bip87"),
        keyorigin(&cli, "emul", "bip87")
    );
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key elip151 --kind wsh --threshold 2 {xpubs}"));
    let d = get_str(&r, "descriptor");
    sh(&format!("{cli} wallet load --wallet mj -d {d}"));

    // Registering the sw wallet works (no-op)
    sh(&format!("{cli} signer register-multisig -s s1 --wallet mj"));
    // Jade fails though because it does not support elip151 keys
    let err = sh_err(&format!(
        "{cli} signer register-multisig -s emul --wallet mj"
    ));
    assert!(err.contains("Jade Error: Only slip77 master blinding key are supported"));

    // Jade does not support elip151 for singlesig too,
    // but since it assumes that the key is slip77 we can do nothing about it.
    let r = sh(&format!(
        "{cli} signer singlesig-desc -s emul --descriptor-blinding-key elip151 --kind wpkh"
    ));
    let desc_ssj = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load -w ssj -d {desc_ssj}"));
    let err = sh_err(&format!("{cli} wallet address -w ssj -s emul"));
    assert!(err.contains("Mismatching addresses between wallet and jade"));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_3of5() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    sw_signer(&cli, "s1");
    sw_signer(&cli, "s2");
    sw_signer(&cli, "s3");
    sw_signer(&cli, "s4");
    sw_signer(&cli, "s5");

    let signers = &["s1", "s2", "s3", "s4", "s5"];
    multisig_wallet(&cli, "multi", 3, signers, "elip151");

    let _ = fund(&env, &cli, "multi", 1_000_000);

    let r = sh(&format!(
        "{cli} wallet issue --wallet multi --satoshi-asset 1000 --satoshi-token 1"
    ));
    let pset = get_str(&r, "pset");
    let (asset, token) = asset_ids_from_issuance_pset(&cli, "multi", pset);
    let (asset, token) = (&asset, &token);
    complete(&cli, "multi", pset, signers);
    assert_eq!(1000, get_balance(&cli, "multi", asset));
    assert_eq!(1, get_balance(&cli, "multi", token));

    let r = sh(&format!(
        "{cli} wallet reissue --wallet multi --asset {asset} --satoshi-asset 1"
    ));
    complete(&cli, "multi", get_str(&r, "pset"), signers);
    assert_eq!(1001, get_balance(&cli, "multi", asset));
    assert_eq!(1, get_balance(&cli, "multi", token));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_start_errors() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, params, _env, _) = setup_cli(env, false);

    let err = sh_err(&format!("{cli} server start {params}"));
    assert!(err.contains("It is probably already running."));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_send_all() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    sw_signer(&cli, "sw");
    singlesig_wallet(&cli, "w1", "sw", "slip77", "wpkh");
    let signers = &["sw"];

    let _ = fund(&env, &cli, "w1", 1_000_000);

    let node_address = env.elementsd_getnewaddress();
    let r = sh(&format!(
        "{cli} wallet drain -w w1 --address {node_address}"
    ));
    complete(&cli, "w1", get_str(&r, "pset"), signers);
    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    assert_eq!(get_balance(&cli, "w1", policy_asset), 0);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_ct_discount() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    sw_signer(&cli, "sw");
    singlesig_wallet(&cli, "w1", "sw", "slip77", "wpkh");
    let signers = &["sw"];

    let _ = fund(&env, &cli, "w1", 1_000_000);

    let address = env.elementsd_getnewaddress();
    let sats = 1_000;
    let recipient = format!(" --recipient {address}:{sats}");

    // Default (with CT discount)
    let r = sh(&format!("{cli} wallet send -w w1 {recipient}"));
    let pset = get_str(&r, "pset");
    complete(&cli, "w1", pset, signers);
    let r = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    let fee_default = r.get("fee").unwrap().as_u64().unwrap();

    assert_eq!(fee_default, 26);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_amp2() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env, _) = setup_cli(env, false);

    sw_signer(&cli, "sw");
    let err = sh_err(&format!("{cli} amp2 descriptor -s sw"));
    assert!(err.contains("AMP2 methods are not available for this network"));

    let err = sh_err(&format!("{cli} amp2 register -s sw"));
    assert!(err.contains("AMP2 methods are not available for this network"));

    let err = sh_err(&format!("{cli} amp2 cosign -p fake_pset"));
    assert!(err.contains("AMP2 methods are not available for this network"));

    // TODO: proper e2e tests with regtest AMP2

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_utxos() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env, _) = setup_cli(env, false);

    sw_signer(&cli, "s1");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    let (txid, addr) = fund(&env, &cli, "w1", 1_000_000);

    let r = sh(&format!("{cli} wallet utxos --wallet w1"));
    assert_eq!(get_len(&r, "utxos"), 1);
    let utxo = &r.get("utxos").unwrap().as_array().unwrap()[0];
    assert_eq!(
        utxo.get("txid").unwrap().as_str().unwrap(),
        txid.to_string()
    );
    assert_eq!(
        utxo.get("address").unwrap().as_str().unwrap(),
        addr.to_string()
    );
    assert_eq!(utxo.get("value").unwrap().as_u64().unwrap(), 1_000_000);

    sh(&format!("{cli} server stop"));

    t.join().unwrap();
}

#[test]
fn test_esplora_backend() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .build();
    let (t, _tmp, cli, params, env, _) = setup_cli(env, false);

    sw_signer(&cli, "s");
    singlesig_wallet(&cli, "w", "s", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w", 1_000_000);

    assert_eq!(txs(&cli, "w").len(), 1);

    // Stop the server
    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // Start again with a Esplora backend
    let t = {
        let cli = cli.clone();
        let params = params.clone();

        // replace "--server-url tcp://..." (last param)
        // with "--server-url http://... --server-type esplora"
        let s = "--server-url";
        let idx = params.find(s).unwrap();
        let esplora_params = format!(
            "{} {} --server-type esplora",
            &params[..idx + s.len()],
            env.esplora_url()
        );

        std::thread::spawn(move || {
            sh(&format!("{cli} server start {esplora_params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(1000));

    assert_eq!(txs(&cli, "w").len(), 1);
    let _ = fund(&env, &cli, "w", 1_000_000);
    assert_eq!(txs(&cli, "w").len(), 2);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_waterfalls() {
    // TODO: merge/replace with setup_cli
    let env = TestEnvBuilder::from_env().with_waterfalls().build();

    let addr = get_available_addr().unwrap();
    let cli = format!("cli --addr {addr} -n regtest");

    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();
    let url = env.waterfalls_url();
    let params = format!("--datadir {datadir} --server-type waterfalls --server-url {url}");

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

    sw_signer(&cli, "s");
    singlesig_wallet(&cli, "w", "s", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w", 1_000_000);

    assert_eq!(txs(&cli, "w").len(), 1);

    // Stop the server
    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}
