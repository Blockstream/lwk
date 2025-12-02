use crate::test_wollet::*;
use lwk_test_util::*;

#[test]
fn test_prune() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", signer.xpub());

    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc);

    // Fund the wallet and explicitly apply the update with the transaction.
    let address = wallet.address();
    let _ = env.elementsd_sendtoaddress(&address, 100_000, None);

    let mut update = next_tx_update(&mut wallet);
    let size_before = update.serialize().unwrap().len();
    // Update.prune() does not remove the _wallet_ rangeproofs
    update.prune(&wallet.wollet);
    let size_after = update.serialize().unwrap().len();
    assert!(size_after < size_before);
    wallet.wollet.apply_update(update).unwrap();

    // Building transactions requires the wallet rangeproofs
    // (since add_input_rangeproofs still defaults to true)
    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&address, 10_000)
        .unwrap()
        .finish()
        .unwrap();
    let _details = wallet.wollet.get_details(&pset).unwrap();

    wallet.sign(&signer, &mut pset);
    let _txid = wallet.send(&mut pset);

    // Create Wollet pruning witnesses
    // Get transaction and check there are no witnesses
    // Reload from persisted and check there are no witnesses
    // Create transaction
    // Call reunblind
    // Call unblind with utxos
}
