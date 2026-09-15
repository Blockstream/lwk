use std::fs;

use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_state_regression() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let server_url = format!("--server-url {}", &env.electrum_url());
    let addr = get_available_addr().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();
    let cli = format!("cli --addr {addr} -n regtest --datadir {datadir}");
    let params = server_url;

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
    let (t, tmp, cli, params, _env) = setup_cli(env);

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

    sh(&format!("{cli} wallet unload --wallet custody"));
    sh(&format!("{cli} wallet load --wallet custody -d {desc}"));
    let state_path = tmp.path().join("liquid-regtest").join("state.json");
    let state = std::fs::read_to_string(&state_path).unwrap();
    assert!(!state.contains(m));

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

    let state = std::fs::read_to_string(&state_path).unwrap();
    assert!(!state.contains(m));

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
fn test_start_errors() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, params, _env) = setup_cli(env);

    let err = sh_err(&format!("{cli} server start {params}"));
    assert!(err.contains("It is probably already running."));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_local_auth() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, tmp, cli, _params, _env) = setup_cli(env);

    let cookie_path = tmp.path().join("liquid-regtest").join(".cookie");
    let real_cookie = std::fs::read_to_string(&cookie_path).unwrap();

    // a cookie the server never generated must be rejected
    std::fs::write(&cookie_path, "__cookie__:not-the-real-secret").unwrap();
    let err = sh_err(&format!("{cli} signer list"));
    assert!(err.contains("Missing or invalid Authorization header"));

    // no cookie file at all must be rejected too
    std::fs::remove_file(&cookie_path).unwrap();
    let err = sh_err(&format!("{cli} signer list"));
    assert!(err.contains("Missing or invalid Authorization header"));

    // restore the real cookie so the server can be stopped cleanly
    std::fs::write(&cookie_path, real_cookie).unwrap();
    sh(&format!("{cli} signer list"));

    // copy regtest cookie to testnet dir
    let testnet_dir = tmp.path().join("liquid-testnet");
    std::fs::create_dir_all(&testnet_dir).unwrap();
    std::fs::copy(
        tmp.path().join("liquid-regtest").join(".cookie"),
        testnet_dir.join(".cookie"),
    )
    .unwrap();
    let cli_addr = cli.replace(" -n regtest", "");
    let err = sh_err(&format!("{cli_addr} -n testnet wallet list"));
    assert!(err.contains("Inconsistent network"));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}
