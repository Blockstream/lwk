mod test_jade;
mod test_ledger;

use crate::test_jade::jade_setup;
use electrum_client::ScriptStatus;
use elements::bitcoin::{bip32::DerivationPath, XKeyIdentifier};
use elements::encode::deserialize;
use elements::hex::FromHex;
use elements::Transaction;
use lwk_common::Signer;
use lwk_containers::testcontainers::clients::Cli;
use lwk_signer::*;
use lwk_test_util::*;
use lwk_wollet::*;
use std::{collections::HashSet, str::FromStr};

#[test]
fn liquid_send_jade_signer() {
    let docker = Cli::default();
    let jade_init = jade_setup(&docker, TEST_MNEMONIC);
    let xpub_identifier = jade_init.jade.identifier().unwrap();
    let signers = [&AnySigner::Jade(jade_init.jade, xpub_identifier)];
    liquid_send(&signers);
}

#[test]
fn liquid_send_software_signer() {
    let signer = SwSigner::new(TEST_MNEMONIC, false).unwrap();
    let signers: [&AnySigner; 1] = [&AnySigner::Software(signer)];
    liquid_send(&signers);
}

#[test]
fn liquid_issue_jade_signer() {
    let docker = Cli::default();
    let jade_init = jade_setup(&docker, TEST_MNEMONIC);
    let xpub_identifier = jade_init.jade.identifier().unwrap();
    let signers = [&AnySigner::Jade(jade_init.jade, xpub_identifier)];
    liquid_issue(&signers);
}

#[test]
fn liquid_issue_software_signer() {
    let signer = SwSigner::new(TEST_MNEMONIC, false).unwrap();
    let signers = [&AnySigner::Software(signer)];
    liquid_issue(&signers);
}

fn liquid_send(signers: &[&AnySigner]) {
    let server = setup(false);
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!(
        "ct(slip77({}),elwpkh({}/*))",
        slip77_key,
        signers[0].xpub().unwrap()
    );
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);
    let asset = wallet.fund_asset(&server);
    server.generate(1);

    wallet.send_btc(signers, None, None);
    let node_address = server.node_getnewaddress();
    wallet.send_asset(signers, &node_address, &asset, None);
    let node_address1 = server.node_getnewaddress();
    let node_address2 = server.node_getnewaddress();
    wallet.send_many(
        signers,
        &node_address1,
        &asset,
        &node_address2,
        &wallet.policy_asset(),
        None,
    );

    TestWollet::check_persistence(wallet);
}

fn liquid_issue(signers: &[&AnySigner]) {
    let server = setup(false);
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!(
        "ct(slip77({}),elwpkh({}/*))",
        slip77_key,
        signers[0].xpub().unwrap()
    );
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let (asset, _token) = wallet.issueasset(signers, 10, 1, None, None);
    wallet.reissueasset(signers, 10, &asset, None);
    wallet.burnasset(signers, 5, &asset, None);

    TestWollet::check_persistence(wallet);
}

#[test]
fn view() {
    let server = setup(false);
    // "view" descriptor
    let xpub = "tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87";
    let descriptor_blinding_key =
        "1111111111111111111111111111111111111111111111111111111111111111";
    let desc_str = format!("ct({},elwpkh({}/*))", descriptor_blinding_key, xpub);
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);
    let _asset = wallet.fund_asset(&server);

    let descriptor_blinding_key =
        "slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023)";
    let desc_str = format!("ct({},elwpkh({}/*))", descriptor_blinding_key, xpub);
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);
}

#[test]
fn origin() {
    let server = setup(false);
    let signer = generate_signer();
    let fingerprint = signer.fingerprint();
    let path = "84h/1776h/0h";
    let xpub = signer
        .derive_xpub(&DerivationPath::from_str(&format!("m/{path}")).unwrap())
        .unwrap();

    let view_key = generate_view_key();
    let desc_str = format!("ct({view_key},elwpkh([{fingerprint}/{path}]{xpub}/*))");
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    let signers: [&AnySigner; 1] = [&AnySigner::Software(signer)];

    let address = server.node_getnewaddress();

    wallet.fund_btc(&server);
    wallet.send_btc(&signers, None, Some((address, 10_000)));
}

#[test]
fn roundtrip() {
    let server = setup(false);

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
    let xpub51: bitcoin::bip32::Xpub = signer51.xpub();
    let xpub52: bitcoin::bip32::Xpub = signer52.xpub();
    let desc5 = format!("ct({view_key},elwsh(multi(2,{xpub51}/*,{xpub52}/*)))");

    let signer6 = generate_signer();
    let slip77_key = generate_slip77();
    let xpub6: bitcoin::bip32::Xpub = signer6.xpub();
    let desc6 = format!("ct(slip77({slip77_key}),elwpkh({xpub6}/<0;1>/*))");

    let signer7 = generate_signer();
    let desc7 = format!("ct(elip151,elwpkh({}/*))", signer7.xpub());

    let signers1 = [&AnySigner::Software(signer1)];
    let signers2 = [&AnySigner::Software(signer2)];
    let signers3 = [&AnySigner::Software(signer3)];
    let signers4 = [&AnySigner::Software(signer4)];
    let signers5 = [
        &AnySigner::Software(signer51),
        &AnySigner::Software(signer52),
    ];
    let signers6 = [&AnySigner::Software(signer6)];
    let signers7 = [&AnySigner::Software(signer7)];

    std::thread::scope(|s| {
        for (signers, desc) in [
            (&signers1[..], desc1),
            (&signers2[..], desc2),
            (&signers3[..], desc3),
            (&signers4[..], desc4),
            (&signers5[..], desc5),
            (&signers6[..], desc6),
            (&signers7[..], desc7),
        ] {
            let server = &server;
            let wallet = TestWollet::new(&server.electrs.electrum_url, &desc);
            s.spawn(move || {
                roundtrip_inner(wallet, server, signers);
            });
        }
    });
}

fn roundtrip_inner(mut wallet: TestWollet, server: &TestElectrumServer, signers: &[&AnySigner]) {
    wallet.fund_btc(server);
    server.generate(1);
    wallet.send_btc(signers, None, None);
    let (asset, _token) = wallet.issueasset(signers, 100_000, 1, None, None);
    let node_address = server.node_getnewaddress();
    wallet.send_asset(signers, &node_address, &asset, None);
    let node_address1 = server.node_getnewaddress();
    let node_address2 = server.node_getnewaddress();
    wallet.send_many(
        signers,
        &node_address1,
        &asset,
        &node_address2,
        &wallet.policy_asset(),
        None,
    );
    wallet.reissueasset(signers, 10_000, &asset, None);
    wallet.burnasset(signers, 5_000, &asset, None);
    server.generate(2);
}

#[test]
fn unsupported_descriptor() {
    let signer1 = generate_signer();
    let signer2 = generate_signer();
    let xpub1 = signer1.xpub();
    let xpub2 = signer2.xpub();
    let view_key = generate_view_key();
    let desc_p2pkh = format!("ct({view_key},elpkh({xpub1}/*))");
    let desc_p2sh = format!("ct({view_key},elsh(multi(2,{xpub1}/*,{xpub2}/*)))",);
    let desc_p2tr = format!("ct({view_key},eltr({xpub1}/*))");
    let desc_no_wildcard = format!("ct({view_key},elwpkh({xpub1}))");

    let desc_multi_path_1 = format!("ct({view_key},elwpkh({xpub1}/<0;1;2>/*))");
    let desc_multi_path_2 = format!("ct({view_key},elwpkh({xpub1}/<0;1>/0/*))");
    let desc_multi_path_3 = format!("ct({view_key},elwpkh({xpub1}/<1;0>/*))");
    let desc_multi_path_4 = format!("ct({view_key},elwpkh({xpub1}/<0;2>/*))");
    let desc_multi_path_5 = format!("ct({view_key},elwsh(multi(2,{xpub1}/<0;1>/*,{xpub2}/0/*)))");

    for (desc, err) in [
        (desc_p2pkh, Error::UnsupportedDescriptorNonV0),
        (desc_p2sh, Error::UnsupportedDescriptorNonV0),
        (desc_p2tr, Error::UnsupportedDescriptorNonV0),
        (
            desc_no_wildcard,
            Error::UnsupportedDescriptorWithoutWildcard,
        ),
        (desc_multi_path_1, Error::UnsupportedMultipathDescriptor),
        (desc_multi_path_2, Error::UnsupportedMultipathDescriptor),
        (desc_multi_path_3, Error::UnsupportedMultipathDescriptor),
        (desc_multi_path_4, Error::UnsupportedMultipathDescriptor),
        (desc_multi_path_5, Error::UnsupportedMultipathDescriptor),
    ] {
        new_unsupported_wallet(&desc, err);
    }

    let bare_key = "0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904";
    let desc_bare = format!("ct({},elwpkh({}/*))", bare_key, signer1.xpub());
    new_unsupported_wallet(&desc_bare, Error::BlindingBareUnsupported);

    let xprv = generate_xprv();
    let desc_view_multi = format!("ct({}/<0;1>,elwpkh({}))", xprv, signer1.xpub());
    new_unsupported_wallet(&desc_view_multi, Error::BlindingViewMultiUnsupported);

    let desc_view_wildcard = format!("ct({}/*,elwpkh({}))", xprv, signer1.xpub());
    new_unsupported_wallet(&desc_view_wildcard, Error::BlindingViewWildcardUnsupported);
}

#[test]
fn address() {
    let server = setup(false);

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc);

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
    wallet.fund(&server, satoshi, mid_address, None);
    wallet.fund(&server, satoshi, last_address, None);
}

#[test]
fn different_blinding_keys() {
    // Two wallet with same "bitcoin" descriptor but different blinding keys
    let server = setup(false);

    let signer = generate_signer();
    let view_key1 = generate_view_key();
    let view_key2 = generate_view_key();
    let desc1 = format!("ct({},elwpkh({}/*))", view_key1, signer.xpub());
    let desc2 = format!("ct({},elwpkh({}/*))", view_key2, signer.xpub());

    let mut wallet1 = TestWollet::new(&server.electrs.electrum_url, &desc1);
    wallet1.sync();
    assert_eq!(wallet1.address_result(None).index(), 0);
    wallet1.fund_btc(&server);
    assert_eq!(wallet1.address_result(None).index(), 1);

    let mut wallet2 = TestWollet::new(&server.electrs.electrum_url, &desc2);
    wallet2.sync();
    assert_eq!(wallet2.address_result(None).index(), 0);
    wallet2.fund_btc(&server);
    assert_eq!(wallet2.address_result(None).index(), 1);
}

#[test]
fn fee_rate() {
    // Use a fee rate different from the default one
    let fee_rate = Some(200.0);

    let server = setup(false);
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc);
    wallet.fund_btc(&server);
    wallet.send_btc(&signers, fee_rate, None);
    let (asset, _token) = wallet.issueasset(&signers, 100_000, 1, None, fee_rate);
    let node_address = server.node_getnewaddress();
    wallet.send_asset(&signers, &node_address, &asset, fee_rate);
    let node_address1 = server.node_getnewaddress();
    let node_address2 = server.node_getnewaddress();
    wallet.send_many(
        &signers,
        &node_address1,
        &asset,
        &node_address2,
        &wallet.policy_asset(),
        fee_rate,
    );
    wallet.reissueasset(&signers, 10_000, &asset, fee_rate);
    wallet.burnasset(&signers, 5_000, &asset, fee_rate);
}

#[test]
fn contract() {
    // Issue an asset with a contract
    let contract = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":8,\"ticker\":\"TEST\",\"version\":0}";

    let server = setup(false);
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc);
    wallet.fund_btc(&server);
    wallet.send_btc(&signers, None, None);
    let (_asset, _token) = wallet.issueasset(&signers, 100_000, 1, Some(contract), None);

    // Error cases
    let contract_d = "{\"entity\":{\"domain\":\"testcom\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":8,\"ticker\":\"TEST\",\"version\":0}";
    let contract_v = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":8,\"ticker\":\"TEST\",\"version\":1}";
    let contract_p = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":18,\"ticker\":\"TEST\",\"version\":0}";
    let contract_n = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"\",\"precision\":8,\"ticker\":\"TEST\",\"version\":0}";
    let contract_t = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":8,\"ticker\":\"TT\",\"version\":0}";
    let contract_i = "{\"entity\":{\"domain\":\"test.com\"},\"issuer_pubkey\":\"37cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"Test\",\"precision\":8,\"ticker\":\"TEST\",\"version\":0}";

    for (contract, expected) in [
        (contract_d, Error::InvalidDomain),
        (contract_v, Error::InvalidVersion),
        (contract_p, Error::InvalidPrecision),
        (contract_n, Error::InvalidName),
        (contract_t, Error::InvalidTicker),
        (contract_i, Error::InvalidIssuerPubkey),
    ] {
        let err = Contract::from_str(contract).unwrap_err();
        assert_eq!(err.to_string(), expected.to_string());
    }
}

#[test]
fn multiple_descriptors() {
    // Use a different descriptors for the asset and the reissuance token

    let server = setup(false);
    // Asset descriptor and signers
    let signer_a = generate_signer();
    let view_key_a = generate_view_key();
    let desc_a = format!("ct({},elwpkh({}/*))", view_key_a, signer_a.xpub());
    let mut wallet_a = TestWollet::new(&server.electrs.electrum_url, &desc_a);
    // Token descriptor and signers
    let signer_t1 = generate_signer();
    let signer_t2 = generate_signer();
    let view_key_t = generate_view_key();
    let desc_t = format!(
        "ct({},elwsh(multi(2,{}/*,{}/*)))",
        view_key_t,
        signer_t1.xpub(),
        signer_t2.xpub()
    );
    let mut wallet_t = TestWollet::new(&server.electrs.electrum_url, &desc_t);

    // Fund both wallets
    wallet_a.fund_btc(&server);
    wallet_t.fund_btc(&server);

    // Issue an asset, sending the asset to asset wallet, and the token to the token wallet
    let satoshi_a = 100_000;
    let satoshi_t = 1;
    let address_t = wallet_t.address();
    let mut pset = wallet_a
        .tx_builder()
        .issue_asset(satoshi_a, None, satoshi_t, Some(address_t), None)
        .unwrap()
        .finish()
        .unwrap();

    wallet_t.wollet.add_details(&mut pset).unwrap();
    let (asset, token) = &pset.inputs()[0].issuance_ids();
    let details_a = wallet_a.wollet.get_details(&pset).unwrap();
    let details_t = wallet_t.wollet.get_details(&pset).unwrap();
    assert_eq!(
        *details_a.balance.balances.get(asset).unwrap(),
        satoshi_a as i64
    );
    assert_eq!(
        *details_t.balance.balances.get(token).unwrap(),
        satoshi_t as i64
    );
    wallet_a.sign(&signer_a, &mut pset);
    wallet_a.send(&mut pset);
    wallet_t.sync();
    assert_eq!(wallet_a.balance(asset), satoshi_a);
    assert_eq!(wallet_t.balance(token), satoshi_t);

    // Reissue the asset, sending the asset to asset wallet, and keeping the token in the token
    // wallet
    let satoshi_ar = 1_000;
    let address_a = wallet_a.address();

    let mut pset = wallet_t
        .tx_builder()
        .reissue_asset(*asset, satoshi_ar, Some(address_a), None)
        .unwrap()
        .finish()
        .unwrap();

    wallet_a.wollet.add_details(&mut pset).unwrap();
    let details_a = wallet_a.wollet.get_details(&pset).unwrap();
    let details_t = wallet_t.wollet.get_details(&pset).unwrap();
    assert_eq!(
        *details_a.balance.balances.get(asset).unwrap(),
        satoshi_ar as i64
    );
    assert_eq!(*details_t.balance.balances.get(token).unwrap(), 0i64);
    let mut pset_t1 = pset.clone();
    let mut pset_t2 = pset.clone();
    wallet_t.sign(&signer_t1, &mut pset_t1);
    wallet_t.sign(&signer_t2, &mut pset_t2);
    let mut pset = wallet_t.wollet.combine(&vec![pset_t1, pset_t2]).unwrap();
    wallet_t.send(&mut pset);
    wallet_a.sync();
    assert_eq!(wallet_a.balance(asset), satoshi_a + satoshi_ar);
    assert_eq!(wallet_t.balance(token), satoshi_t);

    // Send the reissuance token to another wallet and issue from there
    let signer_nt = generate_signer();
    let view_key_nt = generate_view_key();
    let desc_nt = format!("ct({},elwpkh({}/*))", view_key_nt, signer_nt.xpub());
    let mut wallet_nt = TestWollet::new(&server.electrs.electrum_url, &desc_nt);

    wallet_nt.fund_btc(&server);
    let address_nt = wallet_nt.address();
    let mut pset = wallet_t
        .tx_builder()
        .add_recipient(&address_nt, satoshi_t, *token)
        .unwrap()
        .finish()
        .unwrap();
    wallet_t.sign(&signer_t1, &mut pset);
    wallet_t.sign(&signer_t2, &mut pset);
    wallet_t.send(&mut pset);
    wallet_nt.sync();
    assert_eq!(wallet_nt.balance(token), satoshi_t);
    assert_eq!(wallet_t.balance(token), 0);

    let issuance = wallet_t
        .wollet
        .issuances()
        .unwrap()
        .into_iter()
        .find(|i| !i.is_reissuance)
        .unwrap();
    assert!(wallet_nt
        .wollet
        .transaction(&issuance.txid)
        .unwrap()
        .is_none());
    let issuance_tx = wallet_t
        .wollet
        .transaction(&issuance.txid)
        .unwrap()
        .unwrap()
        .tx
        .clone();
    let address_a = wallet_a.address();
    let mut pset = wallet_nt
        .tx_builder()
        .reissue_asset(*asset, satoshi_ar, Some(address_a), Some(issuance_tx))
        .unwrap()
        .finish()
        .unwrap();
    wallet_nt.sign(&signer_nt, &mut pset);
    wallet_nt.send(&mut pset);
    wallet_a.sync();
    assert_eq!(wallet_nt.balance(token), satoshi_t);
    assert_eq!(wallet_a.balance(asset), satoshi_a + satoshi_ar * 2);
}

#[test]
fn create_pset_error() {
    let server = setup(false);
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc);
    wallet.fund_btc(&server);
    let satoshi_a = 100_000;
    let satoshi_t = 1;
    let (asset, token) = wallet.issueasset(
        &[&AnySigner::Software(signer.clone())],
        satoshi_a,
        satoshi_t,
        None,
        None,
    );
    let asset_str = asset.to_string();

    // Invalid address
    let addressees = vec![UnvalidatedRecipient {
        satoshi: 1_000,
        address: "".to_string(),
        asset: "".to_string(),
    }];
    let err = wallet
        .tx_builder()
        .set_unvalidated_recipients(&addressees)
        .unwrap_err();
    let expected = "base58 error: base58ck data not even long enough for a checksum";
    assert_eq!(err.to_string(), expected);

    // Not confidential address
    let mut address = wallet.address();
    address.blinding_pubkey = None;
    let not_conf_address = address.to_string();
    let addressees = vec![UnvalidatedRecipient {
        satoshi: 1_000,
        address: not_conf_address,
        asset: "".to_string(),
    }];
    let err = wallet
        .tx_builder()
        .set_unvalidated_recipients(&addressees)
        .unwrap_err();
    assert_eq!(err.to_string(), Error::NotConfidentialAddress.to_string());

    let address = wallet.address().to_string();
    // Invalid amount
    let addressees = vec![UnvalidatedRecipient {
        satoshi: 0,
        address: address.clone(),
        asset: "".to_string(),
    }];
    let err = wallet
        .tx_builder()
        .set_unvalidated_recipients(&addressees)
        .unwrap_err();
    assert_eq!(err.to_string(), Error::InvalidAmount.to_string());

    // Invalid asset
    let addressees = vec![UnvalidatedRecipient {
        satoshi: 1_000,
        address: address.clone(),
        asset: "aaaa".to_string(),
    }];
    let _err = wallet
        .tx_builder()
        .set_unvalidated_recipients(&addressees)
        .unwrap_err();
    // TODO uncomment once https://github.com/ElementsProject/rust-elements/issues/189 is resolved
    // assert_eq!(
    //     err.to_string(),
    //     "bad hex string length 4 (expected 64)".to_string()
    // );

    // Insufficient funds
    // Not enough lbtc
    let addressees = vec![UnvalidatedRecipient {
        satoshi: 2_200_000_000_000_000,
        address: address.clone(),
        asset: "".to_string(),
    }];
    let err = wallet
        .tx_builder()
        .set_unvalidated_recipients(&addressees)
        .unwrap()
        .finish()
        .unwrap_err();
    assert_eq!(err.to_string(), Error::InsufficientFunds.to_string());

    // Not enough asset
    let addressees = vec![UnvalidatedRecipient {
        satoshi: satoshi_a + 1,
        address,
        asset: asset_str.to_string(),
    }];
    let err = wallet
        .tx_builder()
        .set_unvalidated_recipients(&addressees)
        .unwrap()
        .finish()
        .unwrap_err();
    assert_eq!(err.to_string(), Error::InsufficientFunds.to_string());

    // Not enough token
    let signer2 = generate_signer();
    let view_key2 = generate_view_key();
    let desc2 = format!("ct({},elwpkh({}/*))", view_key2, signer2.xpub());
    let wallet2 = TestWollet::new(&server.electrs.electrum_url, &desc2);

    // Send token elsewhere
    let address = wallet2.address();
    let mut pset = wallet
        .tx_builder()
        .add_recipient(&address, satoshi_t, token)
        .unwrap()
        .finish()
        .unwrap();

    wallet.sign(&signer, &mut pset);
    wallet.send(&mut pset);

    let err = wallet
        .tx_builder()
        .reissue_asset(asset, satoshi_a, None, None)
        .unwrap()
        .finish()
        .unwrap_err();

    assert_eq!(err.to_string(), Error::InsufficientFunds.to_string());

    // The other wallet is unaware of the issuance transaction,
    // so it can't reissue the asset.
    let err = wallet2
        .tx_builder()
        .reissue_asset(asset, satoshi_a, None, None)
        .unwrap()
        .finish()
        .unwrap_err();
    assert_eq!(err.to_string(), Error::MissingIssuance.to_string());

    // If you pass the issuance transaction it must contain the asset issuance
    let tx_hex = include_str!("../tests/data/usdt-issuance-tx.hex");
    let tx: Transaction = deserialize(&Vec::<u8>::from_hex(tx_hex).unwrap()).unwrap();
    let err = wallet2
        .tx_builder()
        .reissue_asset(asset, satoshi_a, None, Some(tx))
        .unwrap()
        .finish()
        .unwrap_err();
    assert_eq!(err.to_string(), Error::MissingIssuance.to_string());
}

#[test]
fn multisig_flow() {
    // Simulate a multisig workflow
    let server = setup(false);

    // * Multisig Setup: Start
    // We have 2 signers
    let signer1 = generate_signer();

    let docker = Cli::default();
    let jade_init = jade_setup(&docker, TEST_MNEMONIC);

    let signer2 = &jade_init.jade;
    let signer2_xpub = signer2.xpub().unwrap();
    let signer2_fingerprint = signer2_xpub.fingerprint();

    // Someone generates the "view" descriptor blinding key
    let view_key = generate_view_key();

    // A "coordinator" collects the signers xpubs and the descriptor blinding key,
    // then it creates the multisig descriptor
    let desc_str = format!(
        "ct({},elwsh(multi(2,{}/*,{}/*)))",
        view_key,
        signer1.xpub(),
        signer2_xpub
    );
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    // Sharing desc_str grants watch only access to the wallet.
    // Each signer should have access to desc_str to understand how a PSET is affecting the wallet.

    // * Multisig Setup: Complete

    // * Multisig Sign: Start
    // Fund the wallet
    wallet.fund_btc(&server);
    // Create a simple PSET
    let satoshi = 1_000;
    let node_addr = server.node_getnewaddress();
    let pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&node_addr, satoshi)
        .unwrap()
        .finish()
        .unwrap();

    // Send the PSET to each signer
    let mut pset1 = pset.clone();
    let mut pset2 = pset.clone();
    wallet.sign(&signer1, &mut pset1);
    wallet.sign(&signer2, &mut pset2);

    // Collect and combine the PSETs
    let details = wallet.wollet.get_details(&pset).unwrap();
    for idx in 0..pset.n_inputs() {
        // Each input has 2 misaing signatures
        let sig = &details.sig_details[idx];
        assert_eq!(sig.has_signature.len(), 0);
        assert_eq!(sig.missing_signature.len(), 2);
        // Signatures are expected from signer1 and signer2
        let fingerprints: HashSet<_> = sig.missing_signature.iter().map(|(_, (f, _))| f).collect();
        assert!(fingerprints.contains(&signer1.fingerprint()));
        assert!(fingerprints.contains(&signer2_fingerprint));
    }
    let mut pset = wallet.wollet.combine(&vec![pset1, pset2]).unwrap();

    // Finalize and send the PSET
    wallet.send(&mut pset);

    // * Multisig Sign: Complete
}
#[test]
fn jade_sign_wollet_pset() {
    let server = setup(false);
    let mnemonic = TEST_MNEMONIC;
    let signer = SwSigner::new(mnemonic, false).unwrap();
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!("ct(slip77({}),elwpkh({}/*))", slip77_key, signer.xpub());
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let my_addr = wallet.address();

    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&my_addr, 1000)
        .unwrap()
        .finish()
        .unwrap();

    let docker = Cli::default();
    let jade_init = jade_setup(&docker, mnemonic);

    let xpub_identifier = jade_init.jade.identifier().unwrap();
    let jade_signer = AnySigner::Jade(jade_init.jade, xpub_identifier);
    // Compre strings so that we don't get mismatching regtest-testnet networks
    assert_eq!(
        jade_signer.xpub().unwrap().to_string(),
        signer.xpub().to_string()
    );
    assert_eq!(jade_signer.fingerprint().unwrap(), signer.fingerprint());

    let signatures_added = jade_signer.sign(&mut pset).unwrap();
    assert_eq!(signatures_added, 1);

    wallet.send(&mut pset);
}

#[test]
fn jade_single_sig() {
    let server = setup(false);
    let mnemonic = TEST_MNEMONIC;
    let docker = Cli::default();
    let jade_init = jade_setup(&docker, mnemonic);
    let signer = AnySigner::Jade(
        jade_init.jade,
        XKeyIdentifier::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
    );
    let xpub = SwSigner::new(mnemonic, false).unwrap().xpub();

    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!("ct(slip77({}),elwpkh({}/*))", slip77_key, xpub);
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);
    let satoshi_utxo1 = wallet.balance(&wallet.policy_asset());
    wallet.fund_btc(&server);

    let satoshi = satoshi_utxo1 + 1;
    let node_addr = server.node_getnewaddress();

    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&node_addr, satoshi)
        .unwrap()
        .finish()
        .unwrap();

    wallet.sign(&signer, &mut pset);
    wallet.send(&mut pset);
}

#[test]
fn address_status() {
    let server = setup(false);
    let electrum_url = ElectrumUrl::new(&server.electrs.electrum_url, false, false);
    let mut client = ElectrumClient::new(&electrum_url).unwrap();
    let address = server.node_getnewaddress();
    let initial_status = client.address_status(&address).unwrap();
    assert_eq!(initial_status, None);

    server.node_sendtoaddress(&address, 10000, None);

    let new_status = wait_status_change(&mut client, &address, initial_status);

    server.generate(1);

    let last_status = wait_status_change(&mut client, &address, new_status);

    let mut client = ElectrumClient::new(&electrum_url).unwrap();
    let new_client_status = client.address_status(&address).unwrap();
    assert_eq!(last_status, new_client_status);
}

fn wait_status_change(
    client: &mut ElectrumClient,
    address: &elements::Address,
    initial_status: Option<ScriptStatus>,
) -> Option<ScriptStatus> {
    for _ in 0..50 {
        let status = client.address_status(address).unwrap();
        if initial_status != status {
            return status;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("status didn't change");
}

#[cfg(feature = "esplora_wasm")]
#[tokio::test]
async fn test_esplora_wasm_client() {
    let server = setup(true);
    let url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
    let mut client = EsploraWasmClient::new(&url);
    let signer = generate_signer();
    let view_key = generate_view_key();
    let descriptor = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let network = network_regtest();

    let descriptor: WolletDescriptor = descriptor.parse().unwrap();

    let mut wollet = Wollet::new(network, NoPersist::new(), descriptor).unwrap();

    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    wollet.apply_update(update).unwrap();

    let address = wollet.address(None).unwrap();
    let txid = server.node_sendtoaddress(address.address(), 10000, None);

    let update = wait_update_with_txs(&mut client, &wollet).await;
    dbg!(&update);
    wollet.apply_update(update).unwrap();
    let tx = wollet.transaction(&txid).unwrap().unwrap();
    assert!(tx.height.is_none());
    assert!(wollet.tip().timestamp().is_some());

    server.generate(1);
    let update = wait_update_with_txs(&mut client, &wollet).await;
    wollet.apply_update(update).unwrap();
    let tx = wollet.transaction(&txid).unwrap().unwrap();
    assert!(tx.height.is_some());
    assert!(wollet.tip().timestamp().is_some());
}

#[cfg(feature = "esplora_wasm")]
async fn wait_update_with_txs(client: &mut EsploraWasmClient, wollet: &Wollet) -> Update {
    for _ in 0..50 {
        let update = client.full_scan(wollet).await.unwrap();
        if let Some(update) = update {
            if !update.only_tip() {
                return update;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("update didn't arrive");
}

#[test]
fn test_tip() {
    let server = setup(false);
    let mut w = TestWollet::with_test_desc(&server.electrs.electrum_url);
    w.wait_height(101); // node mines 101 blocks on start
    assert_eq!(w.tip().height(), 101);
    assert!(w.tip().timestamp().is_some());
    server.generate(1);
    w.wait_height(102);
    assert_eq!(w.tip().height(), 102);
    assert!(w.tip().timestamp().is_some());
}

#[test]
fn drain() {
    // Send all funds from a wallet
    let server = setup(false);
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc);
    wallet.fund_btc(&server);
    let lbtc = wallet.policy_asset();

    // One utxo L-BTC
    let node_address = server.node_getnewaddress();
    // TODO: move to send all fn
    let balance_before = wallet.balance(&lbtc);

    let mut pset = wallet
        .tx_builder()
        .drain_lbtc_wallet()
        .drain_lbtc_to(node_address)
        .finish()
        .unwrap();

    let details = wallet.wollet.get_details(&pset).unwrap();
    let fee = details.balance.fee as i64;
    assert!(fee > 0);
    assert_eq!(
        *details.balance.balances.get(&lbtc).unwrap(),
        -(balance_before as i64)
    );

    for signer in signers {
        wallet.sign(signer, &mut pset);
    }
    let tx = wallet.wollet.finalize(&mut pset).unwrap();
    let electrum_client = ElectrumClient::new(&wallet.electrum_url).unwrap();
    let _txid = electrum_client.broadcast(&tx).unwrap();
    wallet.sync();
    let balance_after = wallet.balance(&lbtc);
    assert_eq!(balance_after, 0);
    let txs = wallet.wollet.transactions().unwrap();
    assert_eq!(txs.len(), 1);
    // FIXME: should be 2, txs with no outputs belonging to the wallet are not in the list, but
    // they should.
}
