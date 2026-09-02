use std::collections::HashSet;

use serde_json::Value;

use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_3of5() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

    sw_signer(&cli, "s1");
    sw_signer(&cli, "s2");
    sw_signer(&cli, "s3");
    sw_signer(&cli, "s4");
    sw_signer(&cli, "s5");

    let signers = &["s1", "s2", "s3", "s4", "s5"];
    multisig_wallet(&cli, "multi", 3, signers, "elip151");

    let _ = fund(&env, &cli, "multi", 1_000_000);

    let r = sh(&format!(
        "{cli} wallet issue --wallet multi --satoshi-asset 1000 --satoshi-token 1"
    ));
    let pset = get_str(&r, "pset");
    let (asset, token) = asset_ids_from_issuance_pset(&cli, "multi", pset);
    let (asset, token) = (&asset, &token);
    complete(&cli, "multi", pset, signers);
    assert_eq!(1000, get_balance(&cli, "multi", asset));
    assert_eq!(1, get_balance(&cli, "multi", token));

    let r = sh(&format!(
        "{cli} wallet reissue --wallet multi --asset {asset} --satoshi-asset 1"
    ));
    complete(&cli, "multi", get_str(&r, "pset"), signers);
    assert_eq!(1001, get_balance(&cli, "multi", asset));
    assert_eq!(1, get_balance(&cli, "multi", token));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_multisig() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

    sw_signer(&cli, "s1");
    sw_signer(&cli, "s2");

    let r = sh(&format!("{cli} signer xpub --signer s1 --kind bip87"));
    let keyorigin_xpub1 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();
    let r = sh(&format!("{cli} signer xpub --signer s2 --kind bip87"));
    let keyorigin_xpub2 = r.get("keyorigin_xpub").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} wallet multisig-desc --descriptor-blinding-key slip77-rand --kind wsh --threshold 2 --keyorigin-xpub {keyorigin_xpub1} --keyorigin-xpub {keyorigin_xpub2}"));
    let desc = r.get("descriptor").unwrap().as_str().unwrap();
    sh(&format!("{cli} wallet load --wallet multi -d {desc}"));

    let _ = fund(&env, &cli, "multi", 1_000_000);

    let node_address = env.elementsd_getnewaddress();
    let satoshi = 1000;
    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
    let recipient = format!("{node_address}:{satoshi}:{policy_asset}");
    let r = sh(&format!(
        "{cli} wallet send --wallet multi --recipient {recipient}"
    ));
    let pset_u = r.get("pset").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset_u}"));
    let pset_s1 = r.get("pset").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} signer sign --signer s2 --pset {pset_u}"));
    let pset_s2 = r.get("pset").unwrap().as_str().unwrap();

    assert_ne!(pset_u, pset_s1);
    assert_ne!(pset_u, pset_s2);
    assert_ne!(pset_s1, pset_s2);

    let r = sh(&format!(
        "{cli} wallet pset-details --wallet multi -p {pset_u}"
    ));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert!(!r.get("balance").unwrap().as_object().unwrap().is_empty());
    let fee = r
        .get("fees")
        .unwrap()
        .as_object()
        .unwrap()
        .get(policy_asset)
        .unwrap()
        .as_u64()
        .unwrap();
    assert!(fee > 0);
    assert!(r.get("issuances").unwrap().as_array().unwrap().is_empty());
    assert!(r.get("reissuances").unwrap().as_array().unwrap().is_empty());
    let has_sigs = r.get("has_signatures_from").unwrap().as_array().unwrap();
    assert_eq!(has_sigs.len(), 0);
    let missing_sigs = r
        .get("missing_signatures_from")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(missing_sigs.len(), 2);
    let f = |s: &Value| s.get("name").unwrap().as_str().unwrap().to_string();
    let sigs: HashSet<_> = missing_sigs.iter().map(f).collect();
    assert!(sigs.contains("s1"));
    assert!(sigs.contains("s2"));

    let r = sh(&format!(
        "{cli} wallet pset-details --wallet multi -p {pset_s1}"
    ));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert!(!r.get("balance").unwrap().as_object().unwrap().is_empty());
    let fee = r
        .get("fees")
        .unwrap()
        .as_object()
        .unwrap()
        .get(policy_asset)
        .unwrap()
        .as_u64()
        .unwrap();
    assert!(fee > 0);
    assert!(r.get("issuances").unwrap().as_array().unwrap().is_empty());
    assert!(r.get("reissuances").unwrap().as_array().unwrap().is_empty());
    let has_sigs = r.get("has_signatures_from").unwrap().as_array().unwrap();
    assert_eq!(has_sigs.len(), 1);
    assert_eq!(has_sigs[0].get("name").unwrap().as_str().unwrap(), "s1");
    let missing_sigs = r
        .get("missing_signatures_from")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(missing_sigs.len(), 1);
    assert_eq!(missing_sigs[0].get("name").unwrap().as_str().unwrap(), "s2");

    let r = sh(&format!(
        "{cli} wallet pset-details --wallet multi -p {pset_s2}"
    ));
    assert!(r.get("warnings").unwrap().as_str().unwrap().is_empty());
    assert!(!r.get("balance").unwrap().as_object().unwrap().is_empty());
    let fee = r
        .get("fees")
        .unwrap()
        .as_object()
        .unwrap()
        .get(policy_asset)
        .unwrap()
        .as_u64()
        .unwrap();
    assert!(fee > 0);
    assert!(r.get("issuances").unwrap().as_array().unwrap().is_empty());
    assert!(r.get("reissuances").unwrap().as_array().unwrap().is_empty());
    let has_sigs = r.get("has_signatures_from").unwrap().as_array().unwrap();
    assert_eq!(has_sigs.len(), 1);
    assert_eq!(has_sigs[0].get("name").unwrap().as_str().unwrap(), "s2");
    let missing_sigs = r
        .get("missing_signatures_from")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(missing_sigs.len(), 1);
    assert_eq!(missing_sigs[0].get("name").unwrap().as_str().unwrap(), "s1");

    let r = sh(&format!(
        "{cli} wallet combine --wallet multi -p {pset_s1} -p {pset_s2}"
    ));
    let pset_s = r.get("pset").unwrap().as_str().unwrap();

    let r = sh(&format!(
        "{cli} wallet broadcast --wallet multi --pset {pset_s}"
    ));
    let _txid = r.get("txid").unwrap().as_str().unwrap();

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}
