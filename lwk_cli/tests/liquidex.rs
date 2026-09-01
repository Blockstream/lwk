use elements::pset::PartiallySignedTransaction;

use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_liquidex() {
    // Test liquidex swap
    // w1 sell asset issued
    // w2 pay with policy asset

    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";

    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

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

    let r = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    assert!(get_str(&r, "warnings").contains("non-default sighash"));

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
    assert!(get_str(&result, "warnings").is_empty());
    // TODO: check the other fields

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
