use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_signer_load_unload_list() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env) = setup_cli(env);

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
    let (t, _tmp, cli, _params, _env) = setup_cli(env);

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
