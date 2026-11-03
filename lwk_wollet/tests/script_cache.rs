use crate::test_wollet::*;
use elements::{Address, AssetId};
use lwk_common::{PsetDetails, Signer};
use lwk_signer::AnySigner;
use lwk_test_util::*;
use lwk_wollet::{
    full_scan_to_index_with_electrum_client, ElectrumClient, WolletBuilder, WolletDescriptor,
};

fn is_owned(details: &PsetDetails, address: &Address) -> bool {
    let script_pubkey = address.script_pubkey();
    details
        .outputs()
        .iter()
        .find(|o| o.script_pubkey() == &script_pubkey)
        .unwrap()
        .is_owned()
}

fn net_balance(details: &PsetDetails, asset: &AssetId) -> i64 {
    *details.balance().balances().get(asset).unwrap()
}

fn new_counterparty(env: &TestEnv) -> TestWollet<ElectrumClient> {
    let desc = format!(
        "ct({},elwpkh({}/<0;1>/*))",
        generate_view_key(),
        generate_signer().xpub()
    );
    let mut wollet = TestWollet::new(test_client_electrum(&env.electrum_url()), &desc);
    wollet.fund(env, 100_000, None, None);
    wollet
}

#[test]
fn test_unsynced_wallet() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    // B: the counterparty building the PSET
    let b = new_counterparty(&env);

    let desc_a = format!(
        "ct({},elwpkh({}/<0;1>/*))",
        generate_view_key(),
        generate_signer().xpub()
    );

    // Offline: A was never synced, so even the first address is not recognized
    let descriptor: WolletDescriptor = desc_a.parse().unwrap();
    let a_offline = WolletBuilder::new(b.wollet.network(), descriptor)
        .build()
        .unwrap();
    let address = a_offline.address(None).unwrap();
    assert_eq!(address.index(), 0);
    let mut pset = b
        .tx_builder()
        .add_lbtc_recipient(address.address(), 10_000)
        .unwrap()
        .finish()
        .unwrap();
    a_offline.add_details(&mut pset).unwrap();
    let details = a_offline.get_details(&pset).unwrap();
    assert!(!is_owned(&details, address.address()));
}

#[test]
fn test_signing_external_pset() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let signer = AnySigner::Software(generate_signer());
    let desc = signer.wpkh_slip77_descriptor().unwrap();
    let mut a = TestWollet::new(test_client_electrum(&env.electrum_url()), &desc);
    a.fund(&env, 100_000, None, None);
    let policy_asset = a.policy_asset();

    // B: the counterparty building the PSET
    let b = new_counterparty(&env);

    // A hands out an address beyond its scanned range
    let address = a.address_result(Some(100));

    // B spends A's utxo and pays 60_000 back to the handed out address
    let utxo = &a.wollet.utxos().unwrap()[0];
    let external_utxo = a.make_external(utxo);
    let mut pset = b
        .tx_builder()
        .add_lbtc_recipient(address.address(), 60_000)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    // limitation: A doesn't recognize the address it handed out, so its review shows the whole utxo as sent
    a.wollet.add_details(&mut pset).unwrap();
    let details = a.wollet.get_details(&pset).unwrap();
    assert!(!is_owned(&details, address.address()));
    assert_eq!(net_balance(&details, &policy_asset), -100_000);
    a.sign(&signer, &mut pset);

    // Emulate caching by manual scan
    full_scan_to_index_with_electrum_client(&mut a.wollet, 101, &mut a.client).unwrap();

    // Expected with caching:
    a.wollet.add_details(&mut pset).unwrap();
    let details = a.wollet.get_details(&pset).unwrap();
    assert!(is_owned(&details, address.address()));
    assert_eq!(net_balance(&details, &policy_asset), -40_000);
}

#[test]
fn test_handed_out_address_multisig() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let signer1 = AnySigner::Software(generate_signer());
    let signer2 = AnySigner::Software(generate_signer());
    let desc = multisig_desc(&[&signer1, &signer2], 2);

    // Each cosigner runs its own instance of the same 2-of-2 wallet
    let mut a1 = TestWollet::new(test_client_electrum(&env.electrum_url()), &desc);
    let mut a2 = TestWollet::new(test_client_electrum(&env.electrum_url()), &desc);
    a1.fund(&env, 100_000, None, None);
    a2.sync();
    let policy_asset = a1.policy_asset();

    // B: the counterparty building the PSET
    let b = new_counterparty(&env);

    // Cosigner 1 hands out an address beyond the scanned range
    let address = a1.address_result(Some(100));

    // B spends the multisig utxo and pays 60_000 back to the handed out address
    let utxo = &a1.wollet.utxos().unwrap()[0];
    let external_utxo = a1.make_external(utxo);
    let pset = b
        .tx_builder()
        .add_lbtc_recipient(address.address(), 60_000)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    // Emulate caching of a1 by manual scan
    full_scan_to_index_with_electrum_client(&mut a1.wollet, 101, &mut a1.client).unwrap();

    // Cosigner 1 recognizes output as own since it cached
    let mut pset1 = pset.clone();
    a1.wollet.add_details(&mut pset1).unwrap();
    let details = a1.wollet.get_details(&pset1).unwrap();
    assert!(is_owned(&details, address.address()));
    assert_eq!(net_balance(&details, &policy_asset), -40_000);

    // limitation: cosigner 2 is synced on the same chain, but did not hand out the address,
    // so there is nothing to cache
    let mut pset2 = pset.clone();
    a2.wollet.add_details(&mut pset2).unwrap();
    let details = a2.wollet.get_details(&pset2).unwrap();
    assert!(!is_owned(&details, address.address()));
    assert_eq!(net_balance(&details, &policy_asset), -100_000);

    // Cosigner 2 recognizes the address only through the hints added by cosigner 1
    let details = a2.wollet.get_details(&pset1).unwrap();
    assert!(is_owned(&details, address.address()));
    assert_eq!(net_balance(&details, &policy_asset), -40_000);

    a1.sign(&signer1, &mut pset1);
    a2.sign(&signer2, &mut pset1);
}
