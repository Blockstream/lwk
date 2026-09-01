use lwk_test_util::{TestEnv, TestEnvBuilder};

use crate::common::*;

#[test]
fn test_auth_err() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let addr = get_available_addr().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();
    let server_url = format!("--server-url {}", &env.electrum_url());
    let cli = format!("cli --addr {addr} -n regtest --datadir {datadir}");
    let params = server_url;

    // Blockstream auth requires all three fields set together (regardless of server type).
    let err = sh_err(&format!(
        "{cli} server start {params} --auth-token-url https://login --auth-client-id client_id"
    ));
    assert!(err.contains("All of the following must be set together for authentication:"))
}

#[test]
fn test_auth_success() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .build();
    let addr = get_available_addr().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();
    let cli = format!("cli --addr {addr} -n regtest --datadir {datadir}");
    let params = format!(
        "--server-url {} --server-type esplora --auth-static-token token",
        env.esplora_url()
    );

    let t = {
        let cli = cli.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    let params = format!(
        "--server-url {} --server-type esplora --auth-token-url https://login --auth-client-id client_id --auth-client-secret secret",
        env.esplora_url()
    );

    let t = {
        let cli = cli.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // Auth is also accepted with the Electrum server type (a Blockstream OAuth provider on
    // Electrum needs the `electrum_oidc` feature, enabled for lwk_app). A token over a
    // plaintext `tcp://` url additionally needs `--auth-allow-plaintext-with-token`.
    let params = format!(
        "--server-url {} --server-type electrum --auth-token-url https://login --auth-client-id client_id --auth-client-secret secret --auth-allow-plaintext-with-token",
        env.electrum_url()
    );

    let t = {
        let cli = cli.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

/// Drive one authenticated backend end to end through the CLI: start the server against the auth
/// proxy with a Blockstream token provider, fund a wallet (which forces a scan through the
/// gateway), and assert the funded balance. Seeing the balance proves the OAuth token was accepted
/// end to end, not just that the arguments parse (as in `test_auth_success`). `extra_flags` carries
/// per-transport options, such as electrum's `--auth-allow-plaintext-with-token`.
fn assert_auth_cli_e2e(env: &TestEnv, server_type: &str, extra_flags: &str) {
    let server_url = match server_type {
        "electrum" => env.electrum_url(),
        "esplora" => env.esplora_url(),
        "waterfalls" => env.waterfalls_url(),
        other => panic!("unknown server type {other}"),
    };
    let addr = get_available_addr().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let datadir = tmp.path().display().to_string();
    let cli = format!("cli --addr {addr} -n regtest --datadir {datadir}");
    let params = format!(
        "--scanning-interval 1 --server-url {server_url} \
         --server-type {server_type} --auth-token-url {} --auth-client-id {} \
         --auth-client-secret {} {extra_flags}",
        env.oidc_token_url(),
        lwk_test_util::AUTH_CLIENT_ID,
        lwk_test_util::AUTH_CLIENT_SECRET,
    );

    let t = {
        let cli = cli.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    sw_signer(&cli, "s1");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    let _ = fund(env, &cli, "w1", 1_000_000);

    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    assert_eq!(1_000_000, get_balance(&cli, "w1", policy_asset));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

/// End-to-end authenticated Electrum through the CLI. The gateway is a localhost `tcp://` proxy,
/// so the token additionally needs `--auth-allow-plaintext-with-token`.
#[test]
#[ignore = "requires docker and the rpcproxy image (auth stack)"]
fn test_auth_electrum_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_auth()
        .build();
    assert_auth_cli_e2e(&env, "electrum", "--auth-allow-plaintext-with-token");
}

/// End-to-end authenticated Esplora through the CLI. Esplora sends the token over the (http)
/// connection, so no `--auth-allow-plaintext-with-token`.
#[test]
#[ignore = "requires docker and the blockstream/apisix image (auth stack)"]
fn test_auth_esplora_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_esplora()
        .with_auth()
        .build();
    assert_auth_cli_e2e(&env, "esplora", "");
}

/// End-to-end authenticated Waterfalls through the CLI (the production plugin chain with credit
/// checking). Waterfalls sends the token over the (http) connection, so no
/// `--auth-allow-plaintext-with-token`.
#[test]
#[ignore = "requires docker and the blockstream/apisix image (auth stack)"]
fn test_auth_waterfalls_authenticated() {
    let env = TestEnvBuilder::from_env()
        .with_waterfalls()
        .with_auth()
        .build();
    assert_auth_cli_e2e(&env, "waterfalls", "");
}
