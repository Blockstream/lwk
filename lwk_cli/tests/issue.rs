use elements::encode::serialize;
use elements::hex::ToHex;
use elements::pset::PartiallySignedTransaction;

use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_issue() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

    let policy_asset = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";

    sw_signer(&cli, "s1");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w1", 1_000_000);

    let r = sh(&format!("{cli} asset contract --domain example.com --issuer-pubkey 035d0f7b0207d9cc68870abfef621692bce082084ed3ca0c1ae432dd12d889be01 --name example --ticker EXMP"));
    let contract = serde_json::to_string(&r).unwrap();
    let r = sh(&format!(
        "{cli} wallet issue --wallet w1 --satoshi-asset 1000 --satoshi-token 1 --contract '{contract}'"
    ));
    let pset = get_str(&r, "pset");
    let pset_unsigned: PartiallySignedTransaction = pset.parse().unwrap();

    let r = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    let txid = r.get("txid").unwrap().as_str().unwrap();
    assert!(get_str(&r, "warnings").is_empty());
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
    assert_eq!(get_len(&r, "reissuances"), 0);
    let issuances = r.get("issuances").unwrap().as_array().unwrap();
    assert_eq!(issuances.len(), 1);
    let issuance = &issuances[0].as_object().unwrap();
    assert_eq!(issuance.get("vin").unwrap().as_u64().unwrap(), 0);
    assert!(!issuance.get("is_confidential").unwrap().as_bool().unwrap());
    let asset = issuance.get("asset").unwrap().as_str().unwrap();
    let token = issuance.get("token").unwrap().as_str().unwrap();
    let asset_sats = issuance.get("asset_satoshi").unwrap().as_u64().unwrap();
    let token_sats = issuance.get("token_satoshi").unwrap().as_u64().unwrap();
    assert_eq!(asset_sats, 1000);
    assert_eq!(token_sats, 1);

    let balance = r.get("balance").unwrap().as_object().unwrap();
    // TODO: util to check balance with less unwrap
    assert_eq!(balance.get(asset).unwrap().as_i64().unwrap(), 1000);
    assert_eq!(balance.get(token).unwrap().as_i64().unwrap(), 1);

    let r = sh(&format!(
        "{cli} wallet pset-details --wallet w1 -p {pset} --with-tickers"
    ));
    let balance = r.get("balance").unwrap().as_object().unwrap();
    assert!(balance.get("L-BTC").unwrap().as_i64().unwrap() < 0);

    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let pset_signed: PartiallySignedTransaction = pset.parse().unwrap();

    assert_ne!(pset_signed, pset_unsigned);

    let r = sh(&format!(
        "{cli} wallet broadcast --wallet w1 --pset {pset_signed}"
    ));
    let issuance_txid = get_str(&r, "txid");
    assert_eq!(issuance_txid, txid);
    sh(&format!("{cli} server scan"));

    assert_eq!(get_balance(&cli, "w1", asset), 1000);

    let r = sh(&format!("{cli} wallet balance --wallet w1 --with-tickers"));
    let balance = r.get("balance").unwrap().as_object().unwrap();
    assert!(balance.get("L-BTC").unwrap().as_u64().unwrap() > 0);

    let r = sh(&format!("{cli} asset details --asset {policy_asset}"));
    assert_eq!(get_str(&r, "name"), "liquid bitcoin");
    assert_eq!(get_str(&r, "ticker"), "L-BTC");

    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 1);

    let r = sh(&format!("{cli} wallet tx -w w1 -t {issuance_txid}"));
    let tx = get_str(&r, "tx");
    sh(&format!(
        "{cli} asset insert --asset {asset} --contract '{contract}' --issuance-tx {tx}"
    ));

    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 3);

    let r = sh(&format!("{cli} asset details --asset {asset}"));
    let name = get_str(&r, "name");
    assert_eq!(name, "example");

    let reissuance_token_name = &format!("reissuance token for {name}");
    let r = sh(&format!("{cli} asset details --asset {token}"));
    assert_eq!(get_str(&r, "name"), reissuance_token_name);

    sh(&format!("{cli} asset remove --asset {token}"));
    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 2);

    let asset_balance_pre = get_balance(&cli, "w1", asset);
    let node_address = env.elementsd_getnewaddress();
    let recipient = format!("--recipient {node_address}:1:{asset}");
    let r = sh(&format!("{cli} wallet send --wallet w1 {recipient}"));
    // TODO: add PSET introspection verifying there are asset metadata
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);
    let asset_balance_post = get_balance(&cli, "w1", asset);
    assert_eq!(asset_balance_pre, asset_balance_post + 1);

    let r = sh(&format!(
        "{cli} wallet reissue --wallet w1 --asset {asset} --satoshi-asset 1"
    ));
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);
    assert_eq!(asset_balance_post + 1, get_balance(&cli, "w1", asset));

    let recipient = format!("--recipient burn:1:{asset}");
    let r = sh(&format!("{cli} wallet send --wallet w1 {recipient}"));
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);
    assert_eq!(asset_balance_post, get_balance(&cli, "w1", asset));

    let r = sh(&format!(
        "{cli} wallet burn -w w1 --asset {asset} --satoshi-asset 1"
    ));
    complete(&cli, "w1", get_str(&r, "pset"), &["s1"]);
    assert_eq!(asset_balance_post - 1, get_balance(&cli, "w1", asset));

    let r = sh(&format!("{cli} wallet utxos --wallet w1"));
    assert!(get_len(&r, "utxos") >= 3);

    let r = sh(&format!("{cli} wallet txs --wallet w1"));
    let txs = r.get("txs").unwrap().as_array().unwrap();
    assert!(!txs.is_empty());

    for tx in txs {
        let balance = tx.get("balance").unwrap().as_object().unwrap();
        assert!(balance.get(policy_asset).is_some());

        if tx.get("height").is_some() {
            assert!(tx.get("timestamp").is_some());
        }

        assert!(tx.get("fee").unwrap().as_u64().unwrap() > 0);
        let types = ["issuance", "reissuance", "burn", "incoming", "outgoing"];
        assert!(types.contains(&tx.get("type").unwrap().as_str().unwrap()));
        // Always received or spent L-BTC
        let url = tx.get("unblinded_url").unwrap().as_str().unwrap();
        assert!(url.contains(policy_asset));
    }

    env.elementsd_generate(1);
    sh(&format!("{cli} server scan"));

    let r = sh(&format!("{cli} wallet txs --wallet w1 --with-tickers"));
    let txs = r.get("txs").unwrap().as_array().unwrap();
    assert!(!txs.is_empty());

    for tx in txs {
        assert!(tx.get("height").is_some());
        assert!(tx.get("timestamp").is_some());
    }

    let balance = txs[0].get("balance").unwrap().as_object().unwrap();
    assert!(balance.contains_key("L-BTC"));

    // Move the reissuance token to another wallet and perform an "external" reissuance
    sw_signer(&cli, "s2");
    singlesig_wallet(&cli, "w2", "s2", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w2", 1_000_000);
    let w2_addr = address(&cli, "w2");
    let txid = send(&cli, "w1", &w2_addr, token, 1, &["s1"]);
    wait_tx(&cli, "w2", &txid);
    let r = sh(&format!(
        "{cli} wallet reissue --wallet w2 --asset {asset} --satoshi-asset 1"
    ));
    complete(&cli, "w2", get_str(&r, "pset"), &["s2"]);
    assert_eq!(1, get_balance(&cli, "w2", asset));

    // Reissue from wallet w1 without token fails with MissingReissuanceTokenUtxo
    let err = sh_err(&format!(
        "{cli} wallet reissue --wallet w1 --asset {asset} --satoshi-asset 1"
    ));
    let expected = format!("Reissuance token {token} utxo not found in the wallet");
    assert!(err.contains(&expected));

    // Removing the asset will cause the "external" reissuance to fail
    sh(&format!("{cli} asset remove --asset {asset}"));
    let err = sh_err(&format!(
        "{cli} wallet reissue --wallet w2 --asset {asset} --satoshi-asset 1"
    ));
    assert!(err.contains("Missing issuance"));

    let err = sh_err(&format!("{cli} wallet tx -w w2 -t {issuance_txid}"));
    assert!(err.contains("was not found in wallet 'w2'"));

    // w2 can get the tx from the explorer
    sh(&format!("{cli} wallet tx -w w2 -t {issuance_txid} --fetch"));

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}

#[test]
fn test_registry_publish() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .with_registry()
        .build();
    let (t, _tmp, cli, _params, env) = setup_cli(env);

    sw_signer(&cli, "s1");
    singlesig_wallet(&cli, "w1", "s1", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w1", 1_000_000);

    let r = sh(&format!("{cli} asset contract --domain liquidtestnet.com --issuer-pubkey 035d0f7b0207d9cc68870abfef621692bce082084ed3ca0c1ae432dd12d889be01 --name example --ticker EXMP"));
    let contract = serde_json::to_string(&r).unwrap();
    let r = sh(&format!(
        "{cli} wallet issue --wallet w1 --satoshi-asset 1000 --satoshi-token 1 --contract '{contract}'"
    ));
    let pset = get_str(&r, "pset");

    let r = sh(&format!("{cli} wallet pset-details --wallet w1 -p {pset}"));
    let issuances = r.get("issuances").unwrap().as_array().unwrap();
    let issuance = &issuances[0].as_object().unwrap();
    let asset = issuance.get("asset").unwrap().as_str().unwrap();
    let token = issuance.get("token").unwrap().as_str().unwrap();

    let r = sh(&format!("{cli} signer sign --signer s1 --pset {pset}"));
    let pset = r.get("pset").unwrap().as_str().unwrap();
    let pset_signed: PartiallySignedTransaction = pset.parse().unwrap();

    sh(&format!(
        "{cli} wallet broadcast --wallet w1 --pset {pset_signed}"
    ));

    env.elementsd_generate(2);
    wait_ms(6_000); // otherwise registry may find the issuance tx unconfirmed, wait_tx is not enough

    sh(&format!("{cli} server scan"));

    let tx = serialize(&pset_signed.extract_tx().unwrap()).to_hex();
    sh(&format!(
        "{cli} asset insert --asset {asset} --contract '{contract}' --issuance-tx {tx}"
    ));
    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 3);

    let r = sh(&format!("{cli} asset publish --asset {asset}"));
    assert_eq!(get_str(&r, "asset_id"), asset);

    sh(&format!("{cli} asset remove --asset {asset}"));

    sh(&format!("{cli} asset remove --asset {token}"));

    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 1);

    sh(&format!("{cli} asset from-registry --asset {asset}"));

    let r = sh(&format!("{cli} asset list"));
    assert_eq!(get_len(&r, "assets"), 3);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}
