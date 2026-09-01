use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_send_all() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

    sw_signer(&cli, "sw");
    singlesig_wallet(&cli, "w1", "sw", "slip77", "wpkh");
    let signers = &["sw"];

    let _ = fund(&env, &cli, "w1", 1_000_000);

    let testnet_addr = "tlq1qq2xvpcvfup5j8zscjq05u2wxxjcyewk7979f3mmz5l7uw5pqmx6xf5xy50hsn6vhkm5euwt72x878eq6zxx2z58hd7zrsg9qn";
    let err = sh_err(&format!(
        "{cli} wallet drain -w w1 --address {testnet_addr}"
    ));
    assert!(err.contains("Invalid network"));

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
    let (t, _tmp, cli, _params, env) = setup_cli(env);

    sw_signer(&cli, "sw");
    singlesig_wallet(&cli, "w1", "sw", "slip77", "wpkh");
    let signers = &["sw"];

    let _ = fund(&env, &cli, "w1", 1_000_000);

    let address = env.elementsd_getnewaddress();
    let sats = 1_000;
    let recipient = format!(" --recipient {address}:{sats}");
    let policy_asset_id = env.elementsd_policy_asset().to_string();

    // Default (with CT discount)
    let r = sh(&format!("{cli} wallet send -w w1 {recipient}"));
    let pset = get_str(&r, "pset");
    complete(&cli, "w1", pset, signers);
    let r = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    let fee_default = r
        .get("fees")
        .unwrap()
        .as_object()
        .unwrap()
        .get(&policy_asset_id)
        .unwrap()
        .as_u64()
        .unwrap();

    assert_eq!(fee_default, 26);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}
