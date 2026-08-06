// TODO: move here `drain`, `test_manual_coin_selection` and `test_inputs_order` from `e2e.rs`

use crate::test_wollet::*;
use lwk_signer::AnySigner;
use lwk_test_util::*;

#[test]
fn test_lbtc_coin_selection() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);
    let node_address = env.elementsd_getnewaddress();

    let policy_asset = w.policy_asset();

    // Fund the wallet with 3 L-BTC UTXOs
    w.fund(&env, 100_000, None, None);
    w.fund(&env, 200_000, None, None);
    w.fund(&env, 500_000, None, None);
    env.elementsd_generate(1);
    assert_eq!(w.balance(&policy_asset), 800_000);
    assert_eq!(w.wollet.utxos().unwrap().len(), 3);

    let pset = w
        .tx_builder()
        .add_lbtc_recipient(&node_address, 699_998)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 3); // 2 cover the recipient, the 3rd covers the fee
    assert_fee_rate(compute_fee_rate(&pset), None);

    // A small payment only needs the biggest utxo, not the whole wallet
    let mut pset = w
        .tx_builder()
        .add_lbtc_recipient(&node_address, 1_000)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 1);
    assert_eq!(pset.outputs().len(), 3); // recipient + change + fee
    for signer in signers {
        w.sign(signer, &mut pset);
    }
    w.send(&mut pset);

    assert_eq!(w.wollet.utxos().unwrap().len(), 3); // 2 untouched + the change

    let mut pset = w
        .tx_builder()
        .add_lbtc_recipient(&node_address, 1_000)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 1);
    for signer in signers {
        w.sign(signer, &mut pset);
    }
    w.send(&mut pset);
    env.elementsd_generate(1);

    // Draining still selects all the utxos
    let utxos = w.wollet.utxos().unwrap().len();
    assert_eq!(utxos, 3);

    // `drain_lbtc_to` only sets where the remaining L-BTC goes, it does not select all the utxos
    let pset = w
        .tx_builder()
        .drain_lbtc_to(&node_address)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 1);

    let mut pset = w
        .tx_builder()
        .drain_lbtc_wallet()
        .drain_lbtc_to(&node_address)
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), utxos);
    for signer in signers {
        w.sign(signer, &mut pset);
    }
    w.send(&mut pset);
    assert_eq!(w.balance(&policy_asset), 0);
}
