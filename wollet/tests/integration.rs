mod test_session;

use software_signer::*;
use std::collections::HashSet;
use test_session::*;

#[test]
fn liquid() {
    let mut server = setup();
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let signer = Signer::new(mnemonic, &wollet::EC).unwrap();
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!("ct(slip77({}),elwpkh({}/*))", slip77_key, signer.xpub());
    let mut wallet = TestElectrumWallet::new(&server.electrs.electrum_url, &desc_str);
    let signers = &[&signer];

    wallet.fund_btc(&mut server);
    let asset = wallet.fund_asset(&mut server);

    wallet.send_btc(signers);
    let node_address = server.node_getnewaddress();
    wallet.send_asset(signers, &node_address, &asset);
    let node_address1 = server.node_getnewaddress();
    let node_address2 = server.node_getnewaddress();
    wallet.send_many(
        signers,
        &node_address1,
        &asset,
        &node_address2,
        &wallet.policy_asset(),
    );
    let (asset, _token, _entropy) = wallet.issueasset(signers, 10, 1);
    wallet.reissueasset(signers, 10, &asset);
    wallet.burnasset(signers, 5, &asset);
}

#[test]
fn view() {
    let mut server = setup();
    // "view" descriptor
    let xpub = "tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87";
    let descriptor_blinding_key = "L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q";
    let desc_str = format!("ct({},elwpkh({}/*))", descriptor_blinding_key, xpub);
    let mut wallet = TestElectrumWallet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&mut server);
    let _asset = wallet.fund_asset(&mut server);

    let descriptor_blinding_key =
        "slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023)";
    let desc_str = format!("ct({},elwpkh({}/*))", descriptor_blinding_key, xpub);
    let mut wallet = TestElectrumWallet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&mut server);
}

#[test]
fn roundtrip() {
    let mut server = setup();

    let signer1 = generate_signer();
    let slip77_key = generate_slip77();
    let desc1 = format!("ct(slip77({}),elwpkh({}/*))", slip77_key, signer1.xpub());

    let view_key = generate_view_key();
    let signer2 = generate_signer();
    let desc2 = format!("ct({},elwpkh({}/*))", view_key, signer2.xpub());

    let view_key3 = generate_view_key();
    let signer3 = generate_signer();
    let desc3 = format!("ct({},elsh(wpkh({}/*)))", view_key3, signer3.xpub());

    let view_key = generate_view_key();
    let signer4 = generate_signer();
    let desc4 = format!("ct({},elwpkh({}/9/*))", view_key, signer4.xpub());

    let view_key = generate_view_key();
    let signer51 = generate_signer();
    let signer52 = generate_signer();
    let desc5 = format!(
        "ct({},elwsh(multi(2,{}/*,{}/*)))",
        view_key,
        signer51.xpub(),
        signer52.xpub()
    );

    for (signers, desc) in [
        (vec![&signer1], desc1),
        (vec![&signer2], desc2),
        (vec![&signer3], desc3),
        (vec![&signer4], desc4),
        (vec![&signer51, &signer52], desc5),
    ] {
        let mut wallet = TestElectrumWallet::new(&server.electrs.electrum_url, &desc);
        wallet.fund_btc(&mut server);
        wallet.send_btc(&signers);
        let (asset, _token, _entropy) = wallet.issueasset(&signers, 100_000, 1);
        let node_address = server.node_getnewaddress();
        wallet.send_asset(&signers, &node_address, &asset);
        let node_address1 = server.node_getnewaddress();
        let node_address2 = server.node_getnewaddress();
        wallet.send_many(
            &signers,
            &node_address1,
            &asset,
            &node_address2,
            &wallet.policy_asset(),
        );
        wallet.reissueasset(&signers, 10_000, &asset);
        wallet.burnasset(&signers, 5_000, &asset);
        server.generate(2);
    }
}

#[test]
fn pkh() {
    let mut server = setup();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elpkh({}/*))", view_key, signer.xpub());
    let signers = &[&signer];

    let mut wallet = TestElectrumWallet::new(&server.electrs.electrum_url, &desc);
    wallet.fund_btc(&mut server);
    wallet.send_btc(signers);
    // FIXME: issuance does not work with p2pkh
    //let (_asset, _token, _entropy) = wallet.issueasset(signers, 100_000, 1);
}

#[test]
fn address() {
    let mut server = setup();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let mut wallet = TestElectrumWallet::new(&server.electrs.electrum_url, &desc);

    let gap_limit: u32 = 20;
    let addresses: Vec<_> = (0..(gap_limit + 1))
        .map(|i| wallet.address_result(Some(i)))
        .collect();

    // First unused address has index 0
    let address = wallet.address_result(None);
    assert_eq!(address.index(), 0);
    for i in 0..(gap_limit + 1) {
        assert_eq!(addresses[i as usize].index(), i);
    }

    // We get all different addresses
    let set: HashSet<_> = addresses.iter().map(|a| a.address()).collect();
    assert_eq!(addresses.len(), set.len());

    let max = addresses.iter().map(|a| a.index()).max().unwrap();
    assert_eq!(max, gap_limit);

    // Fund an address beyond the gap limit
    // Note that we need to find and address before it,
    // otherwise the sync mechanism will not look for those funds
    let satoshi = 10_000;
    let mid_address = addresses[(gap_limit / 2) as usize].clone();
    let last_address = addresses[gap_limit as usize].clone();
    assert_eq!(last_address.index(), gap_limit);
    let mid_address = Some(mid_address.address().clone());
    let last_address = Some(last_address.address().clone());
    wallet.fund(&mut server, satoshi, mid_address, None);
    wallet.fund(&mut server, satoshi, last_address, None);
}

#[test]
fn different_blinding_keys() {
    // Two wallet with same "bitcoin" descriptor but different blinding keys
    let mut server = setup();

    let signer = generate_signer();
    let view_key1 = generate_view_key();
    let view_key2 = generate_view_key();
    let desc1 = format!("ct({},elwpkh({}/*))", view_key1, signer.xpub());
    let desc2 = format!("ct({},elwpkh({}/*))", view_key2, signer.xpub());

    let mut wallet1 = TestElectrumWallet::new(&server.electrs.electrum_url, &desc1);
    wallet1.sync();
    assert_eq!(wallet1.address_result(None).index(), 0);
    wallet1.fund_btc(&mut server);
    assert_eq!(wallet1.address_result(None).index(), 1);

    let mut wallet2 = TestElectrumWallet::new(&server.electrs.electrum_url, &desc2);
    wallet2.sync();
    assert_eq!(wallet2.address_result(None).index(), 0);
    wallet2.fund_btc(&mut server);
    assert_eq!(wallet2.address_result(None).index(), 1);
}
