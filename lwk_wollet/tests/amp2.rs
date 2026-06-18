use crate::test_wollet::*;
use lwk_common::{Bip, Signer};
use lwk_test_util::*;
use lwk_wollet::amp2::Amp2;

#[test]
fn test_amp2_flow() {
    let env = TestEnvBuilder::from_env()
        .with_electrum()
        .with_amp2()
        .build();

    let amp2_url = env.amp2_url();

    let resp: serde_json::Value = reqwest::blocking::get(format!("{amp2_url}/info/xpub"))
        .unwrap()
        .json()
        .unwrap();
    let server_keyorigin_xpub = resp["keyorigin_xpub"].as_str().unwrap();

    let amp2 = Amp2::new(server_keyorigin_xpub.to_string(), amp2_url).unwrap();

    let signer = generate_signer();
    let user_keyorigin_xpub = signer.keyorigin_xpub(Bip::Bip87, false).unwrap();
    let view_key = generate_view_key();

    let amp2_desc = amp2
        .descriptor_from_str(&user_keyorigin_xpub, &view_key)
        .unwrap();

    let register_resp = amp2.blocking_register(amp2_desc.clone()).unwrap();
    assert!(!register_resp.wid.is_empty());

    let desc = amp2_desc.descriptor().to_string();
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);

    let lbtc = w.policy_asset();
    w.fund_btc(&env);

    let node_addr = env.elementsd_getnewaddress();
    let mut pset = w
        .tx_builder()
        .add_recipient(&node_addr, 1000, lbtc)
        .unwrap()
        .finish()
        .unwrap();

    let sigs = signer.sign(&mut pset).unwrap();
    assert!(sigs > 0);

    let cosign_resp = amp2.blocking_cosign(&pset).unwrap();
    pset = cosign_resp.pset;

    w.send(&mut pset);
}
