use crate::test_wollet::*;
use lwk_common::derive_blinding_key;
use lwk_test_util::*;
use lwk_wollet::clients::blocking::BlockchainBackend;
use lwk_wollet::*;

// Get the next Update with a transaction
fn next_tx_update<C: BlockchainBackend>(wallet: &mut TestWollet<C>) -> Update {
    for _ in 0..50 {
        if let Some(update) = wallet.client.full_scan(&wallet.wollet).unwrap() {
            if !update.only_tip() {
                return update;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("update didn't arrive");
}

// Sync the wallet, prune the update before applying
fn sync_prune<C: BlockchainBackend>(wallet: &mut TestWollet<C>) {
    let mut update = next_tx_update(wallet);
    let size_before = update.serialize().unwrap().len();
    update.prune(&wallet.wollet);
    let size_after = update.serialize().unwrap().len();
    assert!(size_after < size_before);
    wallet.wollet.apply_update(update).unwrap();
}

#[test]
fn test_prune() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", signer.xpub());

    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc);

    let address = wallet.address();
    let _ = env.elementsd_sendtoaddress(&address, 100_000, None);
    sync_prune(&mut wallet);

    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&address, 10_000)
        .unwrap()
        .finish()
        .unwrap();
    // Update.prune() preserves the input rangeproofs for know wallet outputs
    assert!(pset.inputs()[0].in_utxo_rangeproof.is_some());
    let _details = wallet.wollet.get_details(&pset).unwrap();

    wallet.sign(&signer, &mut pset);
    let tx = wallet.wollet.finalize(&mut pset).unwrap();
    let txid = wallet.client.broadcast(&tx).unwrap();
    sync_prune(&mut wallet);

    let tx = wallet.wollet.transaction(&txid).unwrap().unwrap().tx;
    assert!(tx.input.iter().all(|i| i.witness.is_empty()));
    assert!(tx.output.iter().any(|o| !o.witness.is_empty()));

    // For this wollet we prune all the witnesses
    let mut client = test_client_electrum(&env.electrum_url());
    let tmp_dir = tempfile::TempDir::new().unwrap();

    let network = ElementsNetwork::default_regtest();
    let wd: WolletDescriptor = desc.parse().unwrap();
    let mut w_prune_wit = Wollet::with_fs_persist(network, wd, &tmp_dir).unwrap();

    let mut update = client.full_scan(&w_prune_wit).unwrap().unwrap();
    update.prune_witnesses();
    w_prune_wit.apply_update(update).unwrap();

    let pset = w_prune_wit
        .tx_builder()
        .add_lbtc_recipient(&address, 10_000)
        .unwrap()
        .finish()
        .unwrap();
    // Update.prune_witness() removes all input rangeproofs
    assert!(pset.inputs()[0].in_utxo_rangeproof.is_none());

    let tx = w_prune_wit.transaction(&txid).unwrap().unwrap().tx;
    assert!(tx.input.iter().all(|i| i.witness.is_empty()));
    assert!(tx.output.iter().all(|o| o.witness.is_empty()));

    // reunblind and unblind_utxos_with work on rangeproofs, since there are none,
    // they do nothing
    let outpoints = w_prune_wit.reunblind().unwrap();
    assert_eq!(outpoints.len(), 0);

    let blinding_key =
        derive_blinding_key(w_prune_wit.descriptor().unwrap(), &address.script_pubkey()).unwrap();
    let utxos = w_prune_wit.unblind_utxos_with(blinding_key).unwrap();
    assert_eq!(utxos.len(), 0);
}
