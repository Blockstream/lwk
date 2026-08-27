use lwk_test_util::*;

#[test]
#[ignore = "require ci docker update"]
fn lightningd_getinfo() {
    let env = TestEnvBuilder::from_env()
        .with_bitcoind()
        .with_bitcoincli()
        .with_lightningd()
        .build();

    let info = env.lightningd().client.getinfo().unwrap();
    assert_eq!(info.network, "regtest");
    assert!(info.warning_bitcoind_sync.is_none());
    assert!(info.warning_lightningd_sync.is_none());

    let invoice = env
        .lightningd()
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
}
