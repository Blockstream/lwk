use lwk_containers::testcontainers::clients;
use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_jade_emulator() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

    let docker = clients::Cli::default();
    let test_jade = lwk_jade::TestJadeEmulator::new(&docker);
    let jade_addr = format!("127.0.0.1:{}", test_jade.emulator_port());
    let _guard = test_jade.release_connection();

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
fn test_elip151() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, _env) = setup_cli(env);

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
    let test_jade = lwk_jade::TestJadeEmulator::new(&docker);
    let addr = format!("127.0.0.1:{}", test_jade.emulator_port());
    let _guard = test_jade.release_connection();
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
