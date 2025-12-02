use crate::test_wollet::*;
use lwk_test_util::*;
use lwk_wollet::clients::blocking::BlockchainBackend;

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

    let mut client = test_client_electrum(&env.electrum_url());
    let mut attempts = 50;
    let mut update = loop {
        if let Some(u) = client.full_scan(&wallet.wollet).unwrap() {
            if !u.only_tip() {
                break u;
            }
        }
        attempts -= 1;
        if attempts == 0 {
            panic!("didn't receive an update")
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    let size_before = update.serialize().unwrap().len();
    update.prune(&wallet.wollet);
    let size_after = update.serialize().unwrap().len();
    assert!(size_after < size_before);
    wallet.wollet.apply_update(update).unwrap();

    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&address, 10_000)
        .unwrap()
        .finish()
        .unwrap();
    let _details = wallet.wollet.get_details(&pset).unwrap();

    wallet.sign(&signer, &mut pset);
    wallet.send(&mut pset);
}
