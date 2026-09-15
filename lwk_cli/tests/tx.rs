use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_utxos() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

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
fn test_broadcast() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

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

fn check_blinders(cli: &str, wallet: &str, txid: &str, address: &str, contains: bool) {
    let tx = tx_details(cli, wallet, txid);
    let outputs = tx.get("outputs").unwrap().as_array().unwrap().to_vec();
    let output = outputs
        .iter()
        .find(|o| o.get("address").unwrap().as_str().unwrap() == address)
        .unwrap();
    if contains {
        assert!(!output.get("abf").unwrap().as_str().unwrap().is_empty());
        assert!(!output.get("vbf").unwrap().as_str().unwrap().is_empty());
    } else {
        assert!(output.get("abf").is_none());
        assert!(output.get("vbf").is_none());
    }
}

#[test]
fn test_sent_outputs() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, tmp, cli, params, env) = setup_cli(env);

    sw_signer(&cli, "sw");
    singlesig_wallet(&cli, "w", "sw", "slip77", "wpkh");
    let signers = &["sw"];

    let _ = fund(&env, &cli, "w", 1_000_000);

    let node_address = env.elementsd_getnewaddress();
    let mut node_addr_unconf = node_address.clone();
    node_addr_unconf.blinding_pubkey = None;
    let node_addr_unconf = node_addr_unconf.to_string();
    let sats = 1234;
    let r = sh(&format!(
        "{cli} wallet send -w w --recipient {node_address}:{sats}"
    ));
    let txid = complete(&cli, "w", get_str(&r, "pset"), signers);
    check_blinders(&cli, "w", &txid, &node_addr_unconf, false);

    let err = sh_err(&format!("{cli} wallet dump-unblinded -w w"));
    assert!(err
        .contains("feature not available, start with flag --with-experimental-blinders to enable"));

    // Check last output (fee) script pubkey
    let tx = tx_details(&cli, "w", &txid);
    let outputs = tx.get("outputs").unwrap().as_array().unwrap().to_vec();
    assert_eq!(get_str(outputs.last().unwrap(), "script_pubkey"), "");

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // Restart with experimental blinders
    let t = {
        let cli = cli.clone();
        let params = params.clone();
        std::thread::spawn(move || {
            sh(&format!(
                "{cli} server start {params} --with-experimental-blinders"
            ));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // received, change
    let r = sh(&format!("{cli} wallet dump-unblinded -w w"));
    assert_eq!(get_len(&r, "unblinded"), 2);

    // Send
    let sats = 1001;
    let r = sh(&format!(
        "{cli} wallet send -w w --recipient {node_address}:{sats}"
    ));
    let pset = get_str(&r, "pset");
    let r = sh(&format!("{cli} wallet dump-unblinded -w w"));
    // received, change, sent, change
    assert_eq!(get_len(&r, "unblinded"), 4);
    let txid = complete(&cli, "w", pset, signers);
    check_blinders(&cli, "w", &txid, &node_addr_unconf, true);

    let r = sh(&format!("{cli} wallet dump-unblinded -w w -t {txid}"));
    // sent, change
    assert_eq!(get_len(&r, "unblinded"), 2);

    // Issue
    let sats = 1002;
    let r = sh(&format!(
        "{cli} wallet issue -w w --satoshi-asset {sats} --address-asset {node_address} --satoshi-token 1"
    ));
    let pset = get_str(&r, "pset");
    let (asset, token) = asset_ids_from_issuance_pset(&cli, "w", pset);
    let (asset, _token) = (&asset, &token);
    let txid = complete(&cli, "w", pset, signers);
    check_blinders(&cli, "w", &txid, &node_addr_unconf, true);

    // Reissue
    let sats = 1003;
    let r = sh(&format!(
        "{cli} wallet reissue -w w --asset {asset} --satoshi-asset {sats} --address-asset {node_address}"
    ));
    let txid = complete(&cli, "w", get_str(&r, "pset"), signers);
    check_blinders(&cli, "w", &txid, &node_addr_unconf, true);

    // Drain
    let r = sh(&format!("{cli} wallet drain -w w --address {node_address}"));
    let txid = complete(&cli, "w", get_str(&r, "pset"), signers);
    check_blinders(&cli, "w", &txid, &node_addr_unconf, true);

    // Multiple wallets use the same sqlite db file
    singlesig_wallet(&cli, "shw", "sw", "slip77", "shwpkh");
    let r = sh(&format!("{cli} wallet list"));
    assert_eq!(get_len(&r, "wallets"), 2);
    assert_eq!(txs(&cli, "w").len(), 6);
    assert_eq!(txs(&cli, "shw").len(), 0);

    // lwk_cli/app datadir:
    // └── liquid-testnet
    //     ├── enc_cache             // legacy
    //     │   └── <WALLET "w" HASH>
    //     │       ├── 000000000000  // legacy updates
    //     │       └── 000000000001
    //     ├── .cookie               // RPC auth secret, regenerated on every server start
    //     ├── state.json            // jsonrpc commands to replay on restart (untouched)
    //     └── lwk.sqlite            // sqlite store, for all wallets ("w" and "shw")

    // legacy (no, --with-experimental-blinders)
    let network_dir = tmp.path().join("liquid-regtest");
    let enc_cache = network_dir.join("enc_cache");
    let subdirs: Vec<_> = std::fs::read_dir(&enc_cache)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    // Only wallet "w"
    assert_eq!(subdirs.len(), 1);

    // Both wallet "w" and "shw" use the sqlite file
    assert!(network_dir.join("lwk.sqlite").exists());

    // Every file/dir the server creates should end up owner-only
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;

        let network_dir = tmp.path().join("liquid-regtest");
        assert_eq!(mode(&network_dir), 0o700);
        assert_eq!(mode(&network_dir.join(".cookie")), 0o600);
        assert_eq!(mode(&network_dir.join("state.json")), 0o600);
        assert_eq!(mode(&network_dir.join("lwk.sqlite")), 0o600);
        assert_eq!(mode(&enc_cache), 0o700);
    }

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}
