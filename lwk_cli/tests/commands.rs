use clap::ValueEnum;
use elements::pset::PartiallySignedTransaction;

use lwk_cli::{
    AssetSubCommandsEnum, ServerSubCommandsEnum, SignerSubCommandsEnum, WalletSubCommandsEnum,
};
use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_commands() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

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
fn test_schema() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env) = setup_cli(env);

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
