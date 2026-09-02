use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_wallet_load_unload_list() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env) = setup_cli(env);

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
    let (t, _tmp, cli, params, env) = setup_cli(env);

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
fn test_wallet_details() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env) = setup_cli(env);

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
