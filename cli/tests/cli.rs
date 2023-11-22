use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    str::FromStr,
};

use clap::Parser;
use elements::{pset::PartiallySignedTransaction, Address};
use serde_json::Value;

use cli::{inner_main, Cli};
use test_session::setup;

mod test_session;

/// Returns a non-used local port if available.
///
/// Note there is a race condition during the time the method check availability and the caller
fn get_available_addr() -> anyhow::Result<SocketAddr> {
    // using 0 as port let the system assign a port available
    let t = std::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0))?;
    Ok(t.local_addr()?)
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

#[test]
fn test_start_stop() {
    let addr = get_available_addr().unwrap();
    let t = std::thread::spawn(move || {
        sh(&format!("cli --addr {addr} server start"));
    });
    std::thread::sleep(std::time::Duration::from_millis(100));

    sh(&format!("cli --addr {addr} server stop"));
    t.join().unwrap();
}

#[test]
fn test_signer_load_unload_list() {
    let addr = get_available_addr().unwrap();
    let t = std::thread::spawn(move || {
        sh(&format!("cli --addr {addr} server start"));
    });
    std::thread::sleep(std::time::Duration::from_millis(100));

    let result = sh(&format!("cli --addr {addr} signer list"));
    let signers = result.get("signers").unwrap();
    assert!(signers.as_array().unwrap().is_empty());

    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let result = sh(&format!(
        r#"cli --addr {addr} signer load --kind software --mnemonic "{mnemonic}" --name ss "#
    ));
    assert_eq!(result.get("name").unwrap().as_str().unwrap(), "ss");

    let result = sh(&format!("cli --addr {addr} signer generate"));
    let different_mnemonic = result.get("mnemonic").unwrap().as_str().unwrap();
    let result = sh_result(&format!(
        r#"cli --addr {addr} signer load --kind software --mnemonic "{different_mnemonic}" --name ss"#,
    ));
    assert!(format!("{:?}", result.unwrap_err()).contains("Signer 'ss' is already loaded"));

    let result = sh_result(&format!(
        r#"cli --addr {addr} signer load --kind software --mnemonic "{mnemonic}" --name ss2 "#,
    ));
    assert!(format!("{:?}", result.unwrap_err()).contains("Signer 'ss' is already loaded"));

    let result = sh(&format!("cli --addr {addr} signer list"));
    let signers = result.get("signers").unwrap();
    assert!(!signers.as_array().unwrap().is_empty());

    let result = sh(&format!("cli --addr {addr} signer unload --name ss"));
    let unloaded = result.get("unloaded").unwrap();
    assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "ss");

    let result = sh(&format!("cli --addr {addr} signer list"));
    let signers = result.get("signers").unwrap();
    assert!(signers.as_array().unwrap().is_empty());

    sh(&format!("cli --addr {addr} server stop"));
    t.join().unwrap();
}

#[test]
fn test_wallet_load_unload_list() {
    let addr = get_available_addr().unwrap();
    let t = std::thread::spawn(move || {
        sh(&format!("cli --addr {addr} server start"));
    });
    std::thread::sleep(std::time::Duration::from_millis(100));

    let result = sh(&format!("cli --addr {addr} wallet list"));
    let wallets = result.get("wallets").unwrap();
    assert!(wallets.as_array().unwrap().is_empty());

    let desc = "ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63";
    let result = sh(&format!(
        "cli --addr {addr} wallet load --name custody {desc}"
    ));
    assert_eq!(result.get("descriptor").unwrap().as_str().unwrap(), desc);

    let result = sh_result(&format!(
        "cli --addr {addr} wallet load --name custody anything"
    ));
    assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'custody' is already loaded"));

    let result = sh_result(&format!(
        "cli --addr {addr} wallet load --name differentname {desc}"
    ));
    assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'custody' is already loaded"));

    let result = sh(&format!("cli --addr {addr} wallet list"));
    let wallets = result.get("wallets").unwrap();
    assert!(!wallets.as_array().unwrap().is_empty());

    let result = sh(&format!("cli --addr {addr} wallet unload --name custody"));
    let unloaded = result.get("unloaded").unwrap();
    assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "custody");

    let result = sh(&format!("cli --addr {addr} wallet list"));
    let wallets = result.get("wallets").unwrap();
    assert!(wallets.as_array().unwrap().is_empty());

    sh(&format!("cli --addr {addr} server stop"));
    t.join().unwrap();
}

#[test]
fn test_wallet_details() {
    let server = setup();
    let electrum_url = &server.electrs.electrum_url;
    let addr = get_available_addr().unwrap();
    let options = format!("-n regtest --electrum-url {electrum_url} --addr {addr}");
    let cli = format!("cli {options}");
    let t = std::thread::spawn(move || {
        sh(&format!("cli {options} server start"));
    });
    std::thread::sleep(std::time::Duration::from_millis(100));

    let r = sh(&format!("{cli} signer generate"));
    let m1 = r.get("mnemonic").unwrap().as_str().unwrap();
    let r = sh(&format!(
        "{cli} signer load --kind software --mnemonic \"{m1}\" --name s1"
    ));
    assert_eq!(r.get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!("{cli} signer generate"));
    let m2 = r.get("mnemonic").unwrap().as_str().unwrap();
    let r = sh(&format!(
        "{cli} signer load --kind software --mnemonic \"{m2}\" --name s2"
    ));
    assert_eq!(r.get("name").unwrap().as_str().unwrap(), "s2");

    // Single sig wallet
    let r = sh(&format!(
        "{cli} signer singlesig-desc --name s1 --descriptor-blinding-key slip77 --kind wpkh"
    ));
    let desc_ss = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --name ss {desc_ss}"));

    let r = sh(&format!(
        "{cli} signer singlesig-desc --name s1 --descriptor-blinding-key slip77 --kind shwpkh"
    ));
    let desc_sssh = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --name sssh {desc_sssh}"));

    // Multi sig wallet
    let r = sh(&format!("{cli} signer xpub --name s1 --kind bip84"));
    let xpub1 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} signer xpub --name s2 --kind bip84"));
    let xpub2 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77 --kind wsh --threshold 2 --keyorigin-xpub {xpub1} --keyorigin-xpub {xpub2}"));
    let desc_ms = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --name ms {desc_ms}"));

    // Multi sig wallet, same signers
    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77 --kind wsh --threshold 2 --keyorigin-xpub {xpub1} --keyorigin-xpub {xpub1}"));
    let desc_ms_same_signers = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!(
        "{cli} wallet load --name ms_same_signers {desc_ms_same_signers}"
    ));

    // Details
    let r = sh(&format!("{cli} wallet details --name ss"));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert_eq!(r.get("type").unwrap().as_str().unwrap(), "wpkh");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 1);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!("{cli} wallet details --name sssh"));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert_eq!(r.get("type").unwrap().as_str().unwrap(), "sh_wpkh");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 1);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!("{cli} wallet details --name ms"));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert_eq!(r.get("type").unwrap().as_str().unwrap(), "wsh_multi_2of2");
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 2);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");
    assert_eq!(signers[1].get("name").unwrap().as_str().unwrap(), "s2");

    sh(&format!("{cli} signer unload --name s2"));
    let r = sh(&format!("{cli} wallet details --name ms"));
    let signers = r.get("signers").unwrap().as_array().unwrap();
    assert_eq!(signers.len(), 2);
    assert_eq!(signers[0].get("name").unwrap().as_str().unwrap(), "s1");
    assert!(signers[1].get("name").is_none());

    let r = sh(&format!("{cli} wallet details --name ms_same_signers"));
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
    let server = setup();
    let electrum_url = &server.electrs.electrum_url;
    let addr = get_available_addr().unwrap();
    let options = format!("-n regtest --electrum-url {electrum_url} --addr {addr}");
    let options_clone = options.clone();
    let t = std::thread::spawn(move || {
        sh(&format!("cli {options_clone} server start"));
    });
    std::thread::sleep(std::time::Duration::from_millis(100));

    let result = sh(&format!("cli {options} signer generate"));
    let mnemonic = result.get("mnemonic").unwrap().as_str().unwrap();

    let result = sh(&format!(
        r#"cli {options} signer load --kind software --mnemonic "{mnemonic}" --name s1 "#
    ));
    assert_eq!(result.get("name").unwrap().as_str().unwrap(), "s1");

    let result = sh(&format!(
        r#"cli {options} signer singlesig-desc --name s1 --descriptor-blinding-key slip77 --kind wpkh"#
    ));
    let desc_generated = result.get("descriptor").unwrap().as_str().unwrap();

    let result = sh(&format!(
        r#"cli {options} wallet load --name w1 {desc_generated}"#
    ));
    assert_eq!(
        result.get("descriptor").unwrap().as_str().unwrap(),
        desc_generated
    );

    let result = sh(&format!(r#"cli {options} wallet address --name w1"#));
    let address = result.get("address").unwrap().as_str().unwrap();
    let address = Address::from_str(address).unwrap();

    let _txid = server.node_sendtoaddress(&address, 1_000_000, None);
    server.generate(101);
    std::thread::sleep(std::time::Duration::from_millis(5000)); // TODO poll instead of sleep?

    let regtest_policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let result = sh(&format!("cli {options} wallet balance --name w1"));
    let balance_obj = result.get("balance").unwrap();
    let policy_obj = balance_obj.get(regtest_policy_asset).unwrap();
    assert_eq!(policy_obj.as_number().unwrap().as_u64().unwrap(), 1_000_000);

    let node_address = server.node_getnewaddress();
    let result = sh(&format!(
        r#"cli {options} wallet send --name w1 --recipient {node_address}:1000:{regtest_policy_asset}"#
    ));
    let pset = result.get("pset").unwrap().as_str().unwrap();
    let pset_unsigned: PartiallySignedTransaction = pset.parse().unwrap();

    let result = sh(&format!(r#"cli {options} signer sign --name s1 {pset}"#));
    let pset = result.get("pset").unwrap().as_str().unwrap();
    let pset_signed: PartiallySignedTransaction = pset.parse().unwrap();

    assert_ne!(pset_signed, pset_unsigned);

    let result = sh(&format!(
        r#"cli {options} wallet broadcast --name w1 {pset_signed}"#
    ));
    assert!(result.get("txid").unwrap().as_str().is_some());

    let result = sh(&format!("cli {options} wallet balance --name w1"));
    let balance_obj = result.get("balance").unwrap();
    let policy_obj = balance_obj.get(regtest_policy_asset).unwrap();
    assert!(policy_obj.as_number().unwrap().as_u64().unwrap() < 1_000_000);

    sh(&format!("cli --addr {addr} server stop"));
    t.join().unwrap();
}

#[test]
fn test_issue() {
    // TODO copied from test_broadcast, make an extended setup fn creating a minimal env (1 signer, 1 funded wallt)
    let server = setup();
    let electrum_url = &server.electrs.electrum_url;
    let addr = get_available_addr().unwrap();
    let options = format!("-n regtest --electrum-url {electrum_url} --addr {addr}");
    let options_clone = options.clone();
    let t = std::thread::spawn(move || {
        sh(&format!("cli {options_clone} server start"));
    });
    std::thread::sleep(std::time::Duration::from_millis(100));

    let result = sh(&format!("cli {options} signer generate"));
    let mnemonic = result.get("mnemonic").unwrap().as_str().unwrap();

    let result = sh(&format!(
        r#"cli {options} signer load --kind software --mnemonic "{mnemonic}" --name s1 "#
    ));
    assert_eq!(result.get("name").unwrap().as_str().unwrap(), "s1");

    let result = sh(&format!(
        r#"cli {options} signer singlesig-desc --name s1 --descriptor-blinding-key slip77 --kind wpkh"#
    ));
    let desc_generated = result.get("descriptor").unwrap().as_str().unwrap();

    let result = sh(&format!(
        r#"cli {options} wallet load --name w1 {desc_generated}"#
    ));
    assert_eq!(
        result.get("descriptor").unwrap().as_str().unwrap(),
        desc_generated
    );

    let result = sh(&format!(r#"cli {options} wallet address --name w1"#));
    let address = result.get("address").unwrap().as_str().unwrap();
    let address = Address::from_str(address).unwrap();

    let _txid = server.node_sendtoaddress(&address, 1_000_000, None);
    server.generate(101);
    std::thread::sleep(std::time::Duration::from_millis(15000)); // TODO poll instead of sleep?

    // FIXME, returns thread 'test_issue' panicked at 'called `Result::unwrap()` on an `Err` value: Rpc returned an error RpcError { code: -32005, message: "Wollet Error: Missing vin", data: None }', cli/tests/cli.rs:357:18
    // let result = sh(&format!(
    //     r#"cli {options} wallet issue --name w1 --satoshi-asset 1000 --satoshi-token 0"#
    // ));
    // dbg!(result);

    sh(&format!("cli --addr {addr} server stop"));
    t.join().unwrap();
}

#[test]
fn test_commands() {
    // This test use json `Value` so that changes in the model are noticed

    std::thread::spawn(|| {
        sh("cli server start");
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    let result = sh("cli signer generate");
    assert!(result.get("mnemonic").is_some());

    let desc = "ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63";
    let result = sh(&format!("cli wallet load --name custody {desc}"));
    assert_eq!(result.get("descriptor").unwrap().as_str().unwrap(), desc);

    let result = sh_result("cli wallet load --name wrong wrong");
    assert!(
        format!("{:?}", result.unwrap_err()).contains("Invalid descriptor: Not a CT Descriptor")
    );

    let result = sh("cli wallet balance --name custody");
    let balance_obj = result.get("balance").unwrap();
    let asset = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";
    let policy_obj = balance_obj.get(asset).unwrap();
    assert_eq!(policy_obj.as_number().unwrap().as_u64().unwrap(), 100000);

    let result = sh_result("cli wallet balance --name notexist");
    assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'notexist' does not exist"));

    let result = sh("cli wallet address --name custody");
    assert_eq!(result.get("address").unwrap().as_str().unwrap(), "tlq1qqdtwgfchn6rtl8peyw6afhrkpphqlyxls04vlwycez2fz6l7chlhxr8wtvy9s2v34f9sk0e2g058p0dwdp9kj2z8k7l7ewsnu");
    assert_eq!(result.get("index").unwrap().as_u64().unwrap(), 1);

    let result = sh("cli wallet address --name custody --index 0");
    assert_eq!(result.get("address").unwrap().as_str().unwrap(), "tlq1qqg0nthgrrl4jxeapsa40us5d2wv4ps2y63pxwqpf3zk6y69jderdtzfyr95skyuu3t03sh0fvj09f9xut8erjypuqfev6wuwh");
    assert_eq!(result.get("index").unwrap().as_u64().unwrap(), 0);

    let result = sh("cli wallet send --name custody --recipient tlq1qqwf6dzkyrukfzwmx3cxdpdx2z3zspgda0v7x874cewkucajdzrysa7z9fy0qnjvuz0ymqythd6jxy9d2e8ajka48efakgrp9t:2:144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49");
    let pset = result.get("pset").unwrap().as_str().unwrap();
    let _: PartiallySignedTransaction = pset.parse().unwrap();

    let result = sh("cli wallet unload --name custody");
    let unloaded = result.get("unloaded").unwrap();
    assert_eq!(unloaded.get("descriptor").unwrap().as_str().unwrap(), desc);
    assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "custody");

    let result = sh(
        r#"cli signer load --kind software --mnemonic "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" --name ss "#,
    );
    assert_eq!(result.get("name").unwrap().as_str().unwrap(), "ss");

    let result =
        sh("cli signer singlesig-desc --name ss --descriptor-blinding-key slip77 --kind wpkh");
    let desc_generated = result.get("descriptor").unwrap().as_str().unwrap();

    let result = sh(&format!(
        "cli wallet load --name desc_generated {desc_generated}"
    ));
    let result = result.get("descriptor").unwrap().as_str().unwrap();
    assert_eq!(result, desc_generated);

    let result = sh("cli wallet address --name desc_generated --index 0");
    assert_eq!(result.get("address").unwrap().as_str().unwrap(), "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn");
    assert_eq!(result.get("index").unwrap().as_u64().unwrap(), 0);

    let result = sh("cli signer xpub --name ss --kind bip84");
    let keyorigin_xpub = result.get("keyorigin_xpub").unwrap().as_str().unwrap();
    assert_eq!(keyorigin_xpub, "[73c5da0a/84h/1h/0h]tpubDC8msFGeGuwnKG9Upg7DM2b4DaRqg3CUZa5g8v2SRQ6K4NSkxUgd7HsL2XVWbVm39yBA4LAxysQAm397zwQSQoQgewGiYZqrA9DsP4zbQ1M");

    let result = sh(&format!("cli wallet multisig-desc --descriptor-blinding-key slip77 --kind wsh --threshold 1 --keyorigin-xpub {keyorigin_xpub}"));
    let multisig_desc_generated = result.get("descriptor").unwrap().as_str().unwrap();

    let result = sh(&format!(
        "cli wallet load --name multi_desc_generated {multisig_desc_generated}"
    ));
    let result = result.get("descriptor").unwrap().as_str().unwrap();
    assert_eq!(result, multisig_desc_generated);

    sh("cli server stop");
    std::thread::sleep(std::time::Duration::from_millis(100));
}
