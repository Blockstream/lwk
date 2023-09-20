mod test_session;

use software_signer::*;
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
    let (asset, token, entropy) = wallet.issueasset(signers, 10, 1);
    wallet.reissueasset(signers, 10, &asset, &token, &entropy);
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
        let (asset, token, entropy) = wallet.issueasset(&signers, 100_000, 1);
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
        wallet.reissueasset(&signers, 10_000, &asset, &token, &entropy);
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
