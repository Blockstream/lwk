use clightningrpc::lightningrpc::PayOptions;
use lwk_test_util::*;

#[test]
fn anyswap_ping() {
    let env = TestEnvBuilder::from_env()
        .with_bitcoind()
        .with_bitcoincli()
        .with_esplora()
        .with_bitcoin_esplora()
        .with_lightningd()
        .with_anyswap()
        .build();

    let info = env.lightningd_anyswap().client.getinfo().unwrap();
    assert_eq!(info.network, "regtest");
    assert!(info.warning_bitcoind_sync.is_none());
    assert!(info.warning_lightningd_sync.is_none());
    assert_eq!(info.num_active_channels, 1);

    let invoice = env
        .lightningd_anyswap()
        .client
        .invoice(
            Some(1_000_000),
            "test-label",
            "test-description",
            None,
            None,
            None,
        )
        .unwrap();
    assert!(invoice.bolt11.starts_with("lnbcrt"));
    assert_eq!(invoice.payment_hash.len(), 64);

    // Pay the invoice over the channel, from the plain node to the anyswap node.
    let pay = env
        .lightningd()
        .client
        .pay(&invoice.bolt11, PayOptions::default())
        .unwrap();
    assert_eq!(pay.status, "complete");
    assert_eq!(pay.payment_hash, invoice.payment_hash);
    assert_eq!(pay.amount_msat.0, 1_000_000);

    // Ping anyswap
    let url = format!("{}/v1/info", env.anyswap_url());
    let response = reqwest::blocking::get(&url).unwrap();
    assert_eq!(response.status(), 200);
    let info: serde_json::Value = response.json().unwrap();
    assert_eq!(info["policy"]["protocol_version"], "1.1.0");
    assert_eq!(info["policy"]["network"], "regtest");
}
