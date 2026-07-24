use crate::test_wollet::*;
use lwk_common::*;
use lwk_test_util::*;

#[test]
fn test_assets_owned() {
    let network = Network::default_regtest();
    let policy_asset = *network.policy_asset();
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let client = test_client_electrum(&env.electrum_url());

    let signer = generate_signer();
    let desc = signer.wpkh_slip77_descriptor().unwrap();
    let mut wallet = TestWollet::new(client, &desc);

    wallet.fund_btc(&env);

    // Initially only policy_asset is owned
    let owned = wallet.wollet.assets_owned().unwrap();
    assert_eq!(owned.len(), 1);
    assert!(owned.contains(&policy_asset));

    // Confidential assets
    let asset_id = wallet.fund_asset(&env);

    let owned = wallet.wollet.assets_owned().unwrap();
    assert!(owned.contains(&asset_id));

    // Unconfidential assets
    let asset_id_explicit = env.elementsd_issueasset(100_000);
    env.elementsd_generate(1);
    let _ = wallet.fund_explicit(&env, 10_000, None, Some(asset_id_explicit));

    let owned = wallet.wollet.assets_owned().unwrap();
    assert!(owned.contains(&asset_id_explicit));

    // Spend all assets
    let external_address = env.elementsd_getnewaddress();
    let explicit_utxos = wallet.wollet.explicit_utxos().unwrap();
    let mut pset = wallet
        .tx_builder()
        .add_recipient(&external_address, 10_000, asset_id)
        .unwrap()
        .drain_lbtc_to(external_address)
        .add_external_utxos(explicit_utxos)
        .unwrap()
        .finish()
        .unwrap();

    signer.sign(&mut pset).unwrap();
    wallet.send(&mut pset);

    // assets_owned should still include this assets even after it was fully spent
    let owned = wallet.wollet.assets_owned().unwrap();
    assert!(owned.contains(&asset_id));
    assert!(owned.contains(&asset_id_explicit));
    assert!(owned.contains(&policy_asset));
}
