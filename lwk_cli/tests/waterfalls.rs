use lwk_test_util::TestEnvBuilder;

use crate::common::*;

#[test]
fn test_esplora_waterfalls_backend() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_esplora()
        .with_waterfalls()
        .build();
    let (t, _tmp, cli, params, env) = setup_cli(env);

    // replace "--server-url tcp://..." (last param)
    // with "--server-url http://... --server-type ..."
    let s = "--server-url";
    let idx = params.find(s).unwrap();
    let params_ = &params[..idx + s.len()];
    let url = env.esplora_url();
    let esplora_params = format!("{params_} {url} --server-type esplora");
    let url = env.waterfalls_url();
    let waterfalls_params = format!("{params_} {url} --server-type waterfalls",);

    sw_signer(&cli, "s");
    singlesig_wallet(&cli, "w", "s", "slip77", "wpkh");
    let _ = fund(&env, &cli, "w", 1_000_000);

    assert_eq!(txs(&cli, "w").len(), 1);

    // Stop the server
    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // Start again with a Esplora backend
    let t = {
        let cli = cli.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {esplora_params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    assert_eq!(txs(&cli, "w").len(), 1);
    let _ = fund(&env, &cli, "w", 1_000_000);
    assert_eq!(txs(&cli, "w").len(), 2);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();

    // Start again with a Waterfalls backend
    let t = {
        let cli = cli.clone();
        std::thread::spawn(move || {
            sh(&format!("{cli} server start {waterfalls_params}"));
        })
    };
    std::thread::sleep(std::time::Duration::from_millis(100));

    assert_eq!(txs(&cli, "w").len(), 2);
    let _ = fund(&env, &cli, "w", 1_000_000);
    assert_eq!(txs(&cli, "w").len(), 3);

    sh(&format!("{cli} server stop"));
    t.join().unwrap();
}
