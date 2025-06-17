mod test_jade;
mod test_ledger;
mod test_wollet;

use crate::test_jade::jade_setup;
use clients::blocking::{self, BlockchainBackend};
use electrum_client::ScriptStatus;
use elements::bitcoin::{bip32::DerivationPath, XKeyIdentifier};
use elements::encode::{deserialize, serialize};
use elements::hex::{FromHex, ToHex};
use elements::{OutPoint, Transaction};
use lwk_common::electrum_ssl::{LIQUID_SOCKET, LIQUID_TESTNET_SOCKET};
use lwk_common::Signer;
use lwk_containers::testcontainers::clients::Cli;
use lwk_signer::*;
use lwk_test_util::*;
use lwk_wollet::pegin::fetch_last_full_header;
use lwk_wollet::*;
use std::{collections::HashSet, str::FromStr};
use test_wollet::{generate_signer, test_client_electrum, TestWollet};

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
    let server = setup();
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!(
        "ct(slip77({}),elwpkh({}/*))",
        slip77_key,
        signers[0].xpub().unwrap()
    );
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(&server);
    let asset = wallet.fund_asset(&server);
    server.elementsd_generate(1);

    wallet.send_btc(signers, None, None);
    let node_address = server.elementsd_getnewaddress();
    wallet.send_asset(signers, &node_address, &asset, None);
    let node_address1 = server.elementsd_getnewaddress();
    let node_address2 = server.elementsd_getnewaddress();
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
    let server = setup();
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!(
        "ct(slip77({}),elwpkh({}/*))",
        slip77_key,
        signers[0].xpub().unwrap()
    );
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(&server);

    let (asset, _token) = wallet.issueasset(signers, 10, 1, None, None);
    wallet.reissueasset(signers, 10, &asset, None);
    wallet.burnasset(signers, 5, &asset, None);
    // Issue with 0 amount
    let (_asset, _token) = wallet.issueasset(signers, 0, 1, None, None);

    TestWollet::check_persistence(wallet);
}

#[test]
fn view() {
    let server = setup();
    // "view" descriptor
    let xpub = "tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87";
    let descriptor_blinding_key =
        "1111111111111111111111111111111111111111111111111111111111111111";
    let desc_str = format!("ct({},elwpkh({}/*))", descriptor_blinding_key, xpub);
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(&server);
    let _asset = wallet.fund_asset(&server);

    let descriptor_blinding_key =
        "slip77(9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023)";
    let desc_str = format!("ct({},elwpkh({}/*))", descriptor_blinding_key, xpub);
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(&server);
}

#[test]
fn origin() {
    let server = setup();
    let signer = generate_signer();
    let fingerprint = signer.fingerprint();
    let path = "84h/1776h/0h";
    let xpub = signer
        .derive_xpub(&DerivationPath::from_str(&format!("m/{path}")).unwrap())
        .unwrap();

    let view_key = generate_view_key();
    let desc_str = format!("ct({view_key},elwpkh([{fingerprint}/{path}]{xpub}/*))");
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

    let signers: [&AnySigner; 1] = [&AnySigner::Software(signer)];

    let address = server.elementsd_getnewaddress();

    wallet.fund_btc(&server);
    wallet.send_btc(&signers, None, Some((address, 10_000)));
}

#[test]
fn roundtrip() {
    let server = setup();

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
            let client = test_client_electrum(&server.electrs.electrum_url);
            let wallet = TestWollet::new(client, &desc);
            s.spawn(move || {
                roundtrip_inner(wallet, server, signers);
            });
        }
    });
}

fn roundtrip_inner<C: BlockchainBackend>(
    mut wallet: TestWollet<C>,
    server: &TestElectrumServer,
    signers: &[&AnySigner],
) {
    wallet.fund_btc(server);
    server.elementsd_generate(1);
    wallet.send_btc(signers, None, None);
    let (asset, _token) = wallet.issueasset(signers, 100_000, 1, None, None);
    let node_address = server.elementsd_getnewaddress();
    wallet.send_asset(signers, &node_address, &asset, None);
    let node_address1 = server.elementsd_getnewaddress();
    let node_address2 = server.elementsd_getnewaddress();
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
    server.elementsd_generate(2);
}

#[test]
fn unsupported_descriptor() {
    let signer1 = generate_signer();
    let signer2 = generate_signer();
    let xpub1 = signer1.xpub();
    let xpub2 = signer2.xpub();
    let view_key = generate_view_key();
    let desc_p2pkh = format!("ct({view_key},elpkh({xpub1}/*))");
    let desc_p2sh = format!("ct({view_key},elsh(pkh({xpub1}/*)))",);
    let desc_p2tr = format!("ct({view_key},eltr({xpub1}/*))");

    let desc_multi_path_1 = format!("ct({view_key},elwpkh({xpub1}/<0;1;2>/*))");
    let desc_multi_path_2 = format!("ct({view_key},elwpkh({xpub1}/<0;1>/0/*))");
    let desc_multi_path_3 = format!("ct({view_key},elwpkh({xpub1}/<1;0>/*))");
    let desc_multi_path_4 = format!("ct({view_key},elwpkh({xpub1}/<0;2>/*))");
    let desc_multi_path_5 = format!("ct({view_key},elwsh(multi(2,{xpub1}/<0;1>/*,{xpub2}/0/*)))");

    for (desc, err) in [
        (desc_p2pkh, Error::UnsupportedDescriptorNonV0),
        (desc_p2sh, Error::UnsupportedDescriptorNonV0),
        (desc_p2tr, Error::UnsupportedDescriptorNonV0),
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
    let server = setup();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

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
    wallet.fund(&server, satoshi, mid_address.clone(), None);
    wallet.fund(&server, satoshi, last_address, None);
    let last_unused_before = wallet.address_result(None).index();
    wallet.fund(&server, satoshi, mid_address, None);
    let last_unused_after = wallet.address_result(None).index();
    assert!(
        last_unused_before <= last_unused_after,
        "last_unused_before: {last_unused_before}, last_unused_after: {last_unused_after}"
    );
}

#[test]
fn different_blinding_keys() {
    // Two wallet with same "bitcoin" descriptor but different blinding keys
    let server = setup();

    let signer = generate_signer();
    let view_key1 = generate_view_key();
    let view_key2 = generate_view_key();
    let desc1 = format!("ct({},elwpkh({}/*))", view_key1, signer.xpub());
    let desc2 = format!("ct({},elwpkh({}/*))", view_key2, signer.xpub());

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet1 = TestWollet::new(client, &desc1);
    wallet1.sync();
    assert_eq!(wallet1.address_result(None).index(), 0);
    wallet1.fund_btc(&server);
    assert_eq!(wallet1.address_result(None).index(), 1);

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet2 = TestWollet::new(client, &desc2);
    wallet2.sync();
    assert_eq!(wallet2.address_result(None).index(), 0);
    wallet2.fund_btc(&server);
    assert_eq!(wallet2.address_result(None).index(), 1);

    // Both wallets have 1 tx in the tx list,
    // but since they have the same script pubkey,
    // this case is slightly special and
    // you can get the tx of the other wallet if you query for that specific txid
    wallet1.sync();
    let txs1 = wallet1.wollet.transactions().unwrap();
    assert_eq!(txs1.len(), 1);
    let txid1 = txs1[0].txid;

    let txs2 = wallet2.wollet.transactions().unwrap();
    assert_eq!(txs2.len(), 1);
    let txid2 = txs2[0].txid;

    let tx1_from_w2 = wallet2.wollet.transaction(&txid1).unwrap().unwrap();
    let tx2_from_w1 = wallet1.wollet.transaction(&txid2).unwrap().unwrap();
    assert!(tx1_from_w2.balance.is_empty());
    assert!(tx2_from_w1.balance.is_empty());
    assert_eq!(tx2_from_w1.type_, "unknown");
    assert_eq!(tx1_from_w2.type_, "unknown");
}

#[test]
fn fee_rate() {
    // Use a fee rate different from the default one
    let fee_rate = Some(200.0);

    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);
    wallet.fund_btc(&server);
    wallet.send_btc(&signers, fee_rate, None);
    let (asset, _token) = wallet.issueasset(&signers, 100_000, 1, None, fee_rate);
    let node_address = server.elementsd_getnewaddress();
    wallet.send_asset(&signers, &node_address, &asset, fee_rate);
    let node_address1 = server.elementsd_getnewaddress();
    let node_address2 = server.elementsd_getnewaddress();
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

    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);
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

    let server = setup();
    // Asset descriptor and signers
    let signer_a = generate_signer();
    let view_key_a = generate_view_key();
    let desc_a = format!("ct({},elwpkh({}/*))", view_key_a, signer_a.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet_a = TestWollet::new(client, &desc_a);
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
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet_t = TestWollet::new(client, &desc_t);

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
    assert!(!details_t.balance.balances.contains_key(token));
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
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet_nt = TestWollet::new(client, &desc_nt);

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
    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);
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
    let expected = "base58 error: too short";
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

    // Cannot issue 0 of the asset and 0 of the token
    let err = wallet
        .tx_builder()
        .issue_asset(0, None, 0, None, None)
        .unwrap_err();
    assert!(matches!(err, Error::InvalidAmount));

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
    assert!(matches!(err, Error::InsufficientFunds { .. }));

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
    assert!(matches!(err, Error::InsufficientFunds { .. }));

    // Not enough token
    let signer2 = generate_signer();
    let view_key2 = generate_view_key();
    let desc2 = format!("ct({},elwpkh({}/*))", view_key2, signer2.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let wallet2 = TestWollet::new(client, &desc2);

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

    assert!(matches!(err, Error::InsufficientFunds { .. }));

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
    let server = setup();

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
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

    // Sharing desc_str grants watch only access to the wallet.
    // Each signer should have access to desc_str to understand how a PSET is affecting the wallet.

    // * Multisig Setup: Complete

    // * Multisig Sign: Start
    // Fund the wallet
    wallet.fund_btc(&server);
    // Create a simple PSET
    let satoshi = 1_000;
    let node_addr = server.elementsd_getnewaddress();
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
    let server = setup();
    let mnemonic = TEST_MNEMONIC;
    let signer = SwSigner::new(mnemonic, false).unwrap();
    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!("ct(slip77({}),elwpkh({}/*))", slip77_key, signer.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

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
    let server = setup();
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
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc_str);

    wallet.fund_btc(&server);
    let satoshi_utxo1 = wallet.balance(&wallet.policy_asset());
    wallet.fund_btc(&server);

    let satoshi = satoshi_utxo1 + 1;
    let node_addr = server.elementsd_getnewaddress();

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
    let server = setup();
    let electrum_url = ElectrumUrl::new(&server.electrs.electrum_url, false, false).unwrap();
    let mut client = ElectrumClient::new(&electrum_url).unwrap();
    client.ping().unwrap();
    let address = server.elementsd_getnewaddress();
    let initial_status = client.address_status(&address).unwrap();
    assert_eq!(initial_status, None);

    server.elementsd_sendtoaddress(&address, 10000, None);

    let new_status = wait_status_change(&mut client, &address, initial_status);

    server.elementsd_generate(1);

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

#[cfg(feature = "esplora")]
#[tokio::test]
async fn test_esplora_wasm_client() {
    let server = setup_with_esplora();
    let url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
    let mut client = clients::asyncr::EsploraClient::new(ElementsNetwork::default_regtest(), &url);
    let signer = generate_signer();
    let view_key = generate_view_key();
    let descriptor = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let network = ElementsNetwork::default_regtest();

    let descriptor: WolletDescriptor = descriptor.parse().unwrap();

    let mut wollet = Wollet::new(network, NoPersist::new(), descriptor).unwrap();

    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    wollet.apply_update(update).unwrap();

    let address = wollet.address(None).unwrap();
    let txid = server.elementsd_sendtoaddress(address.address(), 10000, None);

    let update = wait_update_with_txs(&mut client, &wollet).await;
    wollet.apply_update(update).unwrap();
    let tx = wollet.transaction(&txid).unwrap().unwrap();
    assert!(tx.height.is_none());
    assert!(wollet.tip().timestamp().is_some());

    server.elementsd_generate(1);
    let update = wait_update_with_txs(&mut client, &wollet).await;
    wollet.apply_update(update).unwrap();
    let tx = wollet.transaction(&txid).unwrap().unwrap();
    assert!(tx.height.is_some());
    assert!(wollet.tip().timestamp().is_some());
}

#[cfg(feature = "esplora")]
async fn wait_update_with_txs(
    client: &mut clients::asyncr::EsploraClient,
    wollet: &Wollet,
) -> Update {
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

#[ignore = "require network calls"]
#[cfg(feature = "esplora")]
#[tokio::test]
async fn test_esplora_wasm_waterfalls_normal() {
    let url = "https://waterfalls.liquidwebwallet.org/liquid/api";
    let desc = "ct(e350a44c4dad493e7b1faf4ef6a96c1ad13a6fb8d03d61fcec561afb8c3bae18,elwpkh([a8874235/84'/1776'/0']xpub6DLHCiTPg67KE9ksCjNVpVHTRDHzhCSmoBTKzp2K4FxLQwQvvdNzuqxhK2f9gFVCN6Dori7j2JMLeDoB4VqswG7Et9tjqauAvbDmzF8NEPH/<0;1>/*))#3axrmm5c";
    test_esplora_wasm_waterfalls_desc(desc, url).await;
}

#[ignore = "require network calls"]
#[cfg(feature = "esplora")]
#[tokio::test]
async fn test_esplora_wasm_waterfalls_huge() {
    // better to run in release mode
    let url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
    // more than 6k txs
    let desc = "ct(slip77(1bda6cd71a1e206e3eb793e5a4d98a46c3fa473c9ab7bdef9bb9c814764d6614),elwpkh([cb4ba44a/84'/1'/0']tpubDDrybtUajFcgXC85rvwPsh1oU7Azx4kJ9BAiRzMbByqK7UnVXY3gDRJPwEDfaQwguNUZFzrhavJGgEhbsfuebyxUSZQnjLezWVm2Vdqb7UM/<0;1>/*))#za9ktavp";
    test_esplora_wasm_waterfalls_desc(desc, url).await;
}

#[ignore = "require network calls"]
#[cfg(feature = "esplora")]
#[tokio::test]
async fn test_esplora_wasm_waterfalls_missing_txs() {
    let url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
    let desc = "ct(slip77(ae4af8c4fca43888025cb81c0f2b8821c371c1f8340b67b2f75111f9b1cf87f6),elwpkh([43d9b504/84'/1'/0']tpubDCvprDpPNi7AFKbVQjjcvBVdPx7twgpRruwDY8HHtngE7847KjAnusVULdBmKYqvx4hKA9KK2sKMtSSrT5wxndbesxErAWSo8YNQFuhtV8Z/<0;1>/*))#326lmfs5";
    let txs = test_esplora_wasm_waterfalls_desc(desc, url).await;

    let electrum_url = ElectrumUrl::new(LIQUID_TESTNET_SOCKET, true, true).unwrap();
    let mut electrum_client = ElectrumClient::new(&electrum_url).unwrap();
    let desc = WolletDescriptor::from_str(desc).unwrap();
    let mut wollet = Wollet::without_persist(ElementsNetwork::LiquidTestnet, desc).unwrap();
    let update = electrum_client.full_scan(&wollet).unwrap().unwrap();
    wollet.apply_update(update).unwrap();

    assert_eq!(wollet.transactions().unwrap().len(), txs);
}

async fn test_esplora_wasm_waterfalls_desc(desc: &str, url: &str) -> usize {
    let network = if desc.contains("xpub") {
        ElementsNetwork::Liquid
    } else {
        ElementsNetwork::LiquidTestnet
    };

    init_logging();
    use std::time::Instant;

    let desc = WolletDescriptor::from_str(desc).unwrap();

    let mut wollets = vec![];
    for waterfalls in [true, false] {
        let start = Instant::now();
        let mut wollet = Wollet::without_persist(network, desc.clone()).unwrap();
        let mut client = clients::asyncr::EsploraClientBuilder::new(url, network)
            .waterfalls(waterfalls)
            .concurrency(4)
            .build();
        let update = client.full_scan(&wollet).await.unwrap().unwrap();
        wollet.apply_update(update).unwrap();
        let first_scan = start.elapsed();

        println!(
            "waterfall:{waterfalls} first_scan: {}ms {} txs",
            first_scan.as_millis(),
            wollet.transactions().unwrap().len(),
        );

        client.full_scan(&wollet).await.unwrap();
        let second_scan = start.elapsed() - first_scan;

        println!(
            "waterfall:{waterfalls} first_scan: {}ms second_scan: {}ms",
            first_scan.as_millis(),
            second_scan.as_millis()
        );
        wollets.push(wollet);
    }

    assert_eq!(wollets[0].balance().unwrap(), wollets[1].balance().unwrap());
    assert_eq!(
        wollets[0].transactions().unwrap(),
        wollets[1].transactions().unwrap()
    );

    wollets[0].transactions().unwrap().len()
}

#[cfg(feature = "esplora")]
#[tokio::test]
async fn test_esplora_wasm_local_waterfalls() {
    use clients::asyncr::{self, async_sleep};

    init_logging();
    let exe = std::env::var("ELEMENTSD_EXEC").unwrap();
    let test_env = waterfalls::test_env::launch(exe).await;

    let desc = "ct(slip77(ac53739ddde9fdf6bba3dbc51e989b09aa8c9cdce7b7d7eddd49cec86ddf71f7),elwpkh([93970d14/84'/1'/0']tpubDC3BrFCCjXq4jAceV8k6UACxDDJCFb1eb7R7BiKYUGZdNagEhNfJoYtUrRdci9JFs1meiGGModvmNm8PrqkrEjJ6mpt6gA1DRNU8vu7GqXH/<0;1>/*))#u0y4axgs";
    let desc = WolletDescriptor::from_str(desc).unwrap();

    let network = ElementsNetwork::default_regtest();
    let mut wollet = Wollet::without_persist(network, desc.clone()).unwrap();
    let mut client = asyncr::EsploraClientBuilder::new(test_env.base_url(), network)
        .waterfalls(true)
        .build();

    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    wollet.apply_update(update).unwrap();

    assert_eq!(
        format!("{:?}", wollet.balance().unwrap()),
        "{5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225: 0}"
    );

    let address = wollet.address(None).unwrap();
    let txid = test_env.send_to(address.address(), 1_000_000);

    async_sleep(2_000).await;

    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    wollet.apply_update(update).unwrap();

    assert_eq!(
        format!("{:?}", wollet.balance().unwrap()),
        "{5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225: 1000000}"
    );
    let tx = wollet.transaction(&txid).unwrap().unwrap();

    assert!(tx.height.is_none());
    test_env.node_generate(1).await;

    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    wollet.apply_update(update).unwrap();

    let tx = wollet.transaction(&txid).unwrap().unwrap();
    assert_eq!(tx.height.unwrap(), 3);
    let balance = wollet.balance().unwrap();

    let mut wollet =
        Wollet::without_persist(ElementsNetwork::default_regtest(), desc.clone()).unwrap();
    client.avoid_encryption();
    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    wollet.apply_update(update).unwrap();
    assert_eq!(balance, wollet.balance().unwrap());

    // test fallback (no waterfalls) because elip151 desc
    // note the scan will find transactions because the descriptor was used above (with different blinding key)
    let desc = "ct(elip151,elwpkh(tpubDC3BrFCCjXq4jAceV8k6UACxDDJCFb1eb7R7BiKYUGZdNagEhNfJoYtUrRdci9JFs1meiGGModvmNm8PrqkrEjJ6mpt6gA1DRNU8vu7GqXH/<0;1>/*))";
    let desc = WolletDescriptor::from_str(desc).unwrap();
    let mut wollet = Wollet::new(network, NoPersist::new(), desc).unwrap();
    let update = client.full_scan(&wollet).await.unwrap().unwrap();
    wollet.apply_update(update).unwrap();
    assert!(
        wollet.transactions().unwrap().is_empty(),
        "different blinding key should have no txs"
    );
}

#[test]
fn test_tip() {
    let server = setup();
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut w = TestWollet::new(client, TEST_DESCRIPTOR);
    w.wait_height(101); // node mines 101 blocks on start
    assert_eq!(w.tip().height(), 101);
    assert!(w.tip().timestamp().is_some());
    server.elementsd_generate(1);
    w.wait_height(102);
    assert_eq!(w.tip().height(), 102);
    assert!(w.tip().timestamp().is_some());
}

#[test]
fn drain() {
    // Send all funds from a wallet
    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    // One utxo L-BTC
    wallet.fund_btc(&server);
    let node_address = server.elementsd_getnewaddress();
    wallet.assert_spent_unspent(0, 1);
    wallet.send_all_btc(&signers, None, node_address);
    wallet.assert_spent_unspent(1, 0);

    // Multiple utxos
    wallet.fund_btc(&server);
    wallet.fund_btc(&server);
    wallet.assert_spent_unspent(1, 2);

    let node_address = server.elementsd_getnewaddress();
    wallet.send_all_btc(&signers, None, node_address);
    wallet.assert_spent_unspent(3, 0);

    // Drain ignores assets, since their change handling and coin selection is cosiderably easier
    wallet.fund_btc(&server);
    wallet.assert_spent_unspent(3, 1);
    let (asset, token) = wallet.issueasset(&signers, 10, 1, None, None);
    wallet.assert_spent_unspent(4, 3); // unspents are: asset+reissuance_token+change
    let node_address = server.elementsd_getnewaddress();
    wallet.send_all_btc(&signers, None, node_address);
    wallet.assert_spent_unspent(5, 2);

    assert!(wallet.balance(&asset) > 0);
    assert!(wallet.balance(&token) > 0);

    // Confirm the transactions
    server.elementsd_generate(1);
    wait_tx_update(&mut wallet);
    let txs = wallet.wollet.transactions().unwrap();
    for tx in txs {
        assert!(tx.height.is_some());
        assert!(tx.timestamp.is_some());
    }
}

fn wait_tx_update<C: BlockchainBackend>(wallet: &mut TestWollet<C>) {
    for _ in 0..50 {
        if let Some(update) = wallet.client.full_scan(&wallet.wollet).unwrap() {
            if !update.only_tip() {
                wallet.wollet.apply_update(update.clone()).unwrap();

                let err = wallet.wollet.apply_update(update).unwrap_err().to_string();
                assert!(err.starts_with("Update created on a wallet with status"));

                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("update didn't arrive");
}

#[test]
fn ct_discount() {
    // Send transactions with ELIP200 discounted fees for Confidential Transactions
    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signer = AnySigner::Software(signer);

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    wallet.fund_btc(&server);
    let node_address = server.elementsd_getnewaddress();

    // Send without CT discount
    let mut pset = wallet
        .tx_builder()
        .disable_ct_discount()
        .add_lbtc_recipient(&node_address, 1_000)
        .unwrap()
        .finish()
        .unwrap();

    wallet.sign(&signer, &mut pset);
    let details = wallet.wollet.get_details(&pset).unwrap();
    let fee_no_discount = details.balance.fee;
    wallet.send(&mut pset);
    assert_fee_rate(compute_fee_rate_without_discount_ct(&pset), None);

    // Send with CT discount
    let mut pset = wallet
        .tx_builder()
        .enable_ct_discount()
        .add_lbtc_recipient(&node_address, 1_000)
        .unwrap()
        .finish()
        .unwrap();

    wallet.sign(&signer, &mut pset);
    let details = wallet.wollet.get_details(&pset).unwrap();
    let fee_with_discount = details.balance.fee;
    wallet.send(&mut pset);
    assert_fee_rate(compute_fee_rate(&pset), None);

    // Confirm the transactions
    server.elementsd_generate(1);
    wait_tx_update(&mut wallet);
    let txs = wallet.wollet.transactions().unwrap();
    for tx in txs {
        assert!(tx.height.is_some());
        assert!(tx.timestamp.is_some());
    }

    // Check fees
    assert!(fee_no_discount > fee_with_discount);
    assert_eq!(fee_no_discount, 250);
    assert_eq!(fee_with_discount, 26);

    // Default has CT discount enabled
    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&node_address, 1_000)
        .unwrap()
        .finish()
        .unwrap();

    wallet.sign(&signer, &mut pset);
    let details = wallet.wollet.get_details(&pset).unwrap();
    let fee_default = details.balance.fee;
    assert_eq!(fee_with_discount, fee_default);
}

#[test]
fn claim_pegin() {
    // TODO this test makes a pegin using the node as a reference implementation to implement the pegin
    // in the lwk wallet
    let server = setup_with_bitcoind();

    server.bitcoind_generate(101);
    let (mainchain_address, claim_script) = server.elementsd_getpeginaddress();
    let txid = server.bitcoind_sendtoaddress(&mainchain_address, 100_000_000);
    let tx = server.bitcoind_getrawtransaction(txid);
    let tx_bytes = bitcoin::consensus::serialize(&tx);

    let pegin_vout = tx
        .output
        .iter()
        .position(|o| o.script_pubkey == mainchain_address.script_pubkey())
        .unwrap();

    server.bitcoind_generate(101);
    let proof = server.bitcoind_gettxoutproof(txid);

    server.elementsd_generate(2);

    let address_lbtc = server.elementsd_getnewaddress().to_string();

    let inputs = serde_json::json!([ {"txid":txid, "vout": pegin_vout,"pegin_bitcoin_tx": tx_bytes.to_hex(), "pegin_txout_proof": proof, "pegin_claim_script": claim_script } ]);
    let outputs = serde_json::json!([
        {address_lbtc: "0.9", "blinder_index": 0},
        {"fee": "0.1" }
    ]);

    let psbt = server.elementsd_raw_createpsbt(inputs, outputs);

    assert_eq!(server.elementsd_expected_next(&psbt), "updater");
    let psbt = server.elementsd_walletprocesspsbt(&psbt);
    assert_eq!(server.elementsd_expected_next(&psbt), "extractor");
    let tx_hex = server.elementsd_finalizepsbt(&psbt);
    let _txid = server.elementsd_sendrawtransaction(&tx_hex);
}

#[test]
fn test_fetch_full_header_regtest() {
    let server = setup();
    let url = &server.electrs.electrum_url;
    let electrum_url = ElectrumUrl::new(url, false, false).unwrap();
    let client = ElectrumClient::new(&electrum_url).unwrap();

    test_fetch_last_full_header(client, ElementsNetwork::default_regtest());
}

#[test]
fn test_fetch_full_header_mainnet() {
    let electrum_url = ElectrumUrl::new(LIQUID_SOCKET, true, true).unwrap();
    let electrum_client = ElectrumClient::new(&electrum_url).unwrap();
    test_fetch_last_full_header(electrum_client, ElementsNetwork::Liquid);
}

#[test]
fn test_fetch_full_header_testnet() {
    let electrum_url = ElectrumUrl::new(LIQUID_TESTNET_SOCKET, true, true).unwrap();
    let electrum_client = ElectrumClient::new(&electrum_url).unwrap();
    test_fetch_last_full_header(electrum_client, ElementsNetwork::LiquidTestnet);
}

fn test_fetch_last_full_header(mut client: ElectrumClient, network: ElementsNetwork) {
    let current_tip = client.tip().unwrap().height;
    let header = fetch_last_full_header(&client, network, current_tip).unwrap();

    let fed_peg_script = fed_peg_script(&header);
    assert!(fed_peg_script.is_some());
}

#[test]
fn few_lbtc() {
    // Send from a wallet with few lbtc
    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = [&AnySigner::Software(signer)];

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    let address = wallet.address();
    wallet.fund(&server, 1000, Some(address), None);

    let node_address = server.elementsd_getnewaddress();
    wallet.send_btc(&signers, None, Some((node_address, 1)));

    // Drain the wallet and fund it with a single utxo insufficient to pay for the fee
    let node_address = server.elementsd_getnewaddress();
    wallet.send_all_btc(&signers, None, node_address);

    let address = wallet.address();
    wallet.fund(&server, 10, Some(address), None);

    let node_address = server.elementsd_getnewaddress();
    let err = wallet
        .tx_builder()
        .add_lbtc_recipient(&node_address, 1)
        .unwrap()
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::InsufficientFunds { .. }));

    // Send an asset to the wallet and check that we have the same error
    let asset = wallet.fund_asset(&server);
    assert!(wallet.balance(&asset) > 0);

    let err = wallet
        .tx_builder()
        .add_lbtc_recipient(&node_address, 1)
        .unwrap()
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::InsufficientFunds { .. }));

    // Send some more lbtc and we can send the asset and lbtc
    let address = wallet.address();
    wallet.fund(&server, 1000, Some(address), None);
    wallet.send_asset(&signers, &node_address, &asset, None);
    wallet.send_btc(&signers, None, Some((node_address, 1)));
}

pub fn new_unsupported_wallet(desc: &str, expected: lwk_wollet::Error) {
    let r: Result<WolletDescriptor, _> = add_checksum(desc).parse();

    match r {
        Ok(_) => panic!("Expected unsupported descriptor\n{}\n{:?}", desc, expected),
        Err(err) => assert_eq!(err.to_string(), expected.to_string()),
    }
}

#[test]
fn test_prune() {
    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    let address = wallet.address();
    let _ = server.elementsd_sendtoaddress(&address, 100_000, None);

    let electrum_url = ElectrumUrl::new(&server.electrs.electrum_url, false, false).unwrap();
    let mut client = ElectrumClient::new(&electrum_url).unwrap();
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

#[test]
fn test_external_utxo() {
    // Send tx with external utxos
    let server = setup();

    let signer1 = generate_signer();
    let view_key1 = generate_view_key();
    let desc1 = format!("ct({},elwpkh({}/*))", view_key1, signer1.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut w1 = TestWollet::new(client, &desc1);

    let signer2 = generate_signer();
    let view_key2 = generate_view_key();
    let desc2 = format!("ct({},elwpkh({}/*))", view_key2, signer2.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut w2 = TestWollet::new(client, &desc2);

    let policy_asset = w1.policy_asset();

    let address = w1.address();
    w1.fund(&server, 100_000, Some(address), None);

    let address = w2.address();
    w2.fund(&server, 100_000, Some(address), None);

    let utxo = &w2.wollet.utxos().unwrap()[0];
    let external_utxo = w2.make_external(utxo);

    let node_address = server.elementsd_getnewaddress();
    let mut pset = w1
        .tx_builder()
        .add_lbtc_recipient(&node_address, 110_000)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    // Add the details for the extenal wallet to sign
    w2.wollet.add_details(&mut pset).unwrap();
    let details = w1.wollet.get_details(&pset).unwrap();
    assert_eq!(details.sig_details.len(), 2);
    assert_eq!(details.sig_details[0].missing_signature.len(), 1);
    assert_eq!(details.sig_details[1].missing_signature.len(), 1);

    let signers = [&AnySigner::Software(signer1), &AnySigner::Software(signer2)];
    for signer in signers {
        w1.sign(signer, &mut pset);
    }

    let details = w1.wollet.get_details(&pset).unwrap();
    let fee = details.balance.fee;

    w1.send(&mut pset);

    let balance = w1.balance(&policy_asset);
    // utxo w1, utxo w2, sent to node, fee
    assert_eq!(balance, 100_000 + 100_000 - 110_000 - fee);

    // External UTXO can be asset UTXOs
    w2.sync();
    let asset = w2.fund_asset(&server);
    let utxo = &w2.wollet.utxos().unwrap()[0];
    let external_utxo = w2.make_external(utxo);
    assert_eq!(w1.balance(&asset), 0);
    assert_eq!(w2.balance(&asset), 10_000);

    let mut pset = w1
        .tx_builder()
        .add_recipient(&w2.address(), 1_000, asset)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    w2.wollet.add_details(&mut pset).unwrap();
    for signer in signers {
        w1.sign(signer, &mut pset);
    }
    w1.send(&mut pset);
    w2.sync();

    // w1 gets change, w2 gets 1_000
    assert_eq!(w1.balance(&asset), 9_000);
    assert_eq!(w2.balance(&asset), 1_000);

    // Send exact amount (no change) spending only external utxo
    let utxo = &w2.wollet.utxos().unwrap()[0];
    let external_utxo = w2.make_external(utxo);

    let mut pset = w1
        .tx_builder()
        .add_recipient(&w2.address(), 1_000, asset)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    w2.wollet.add_details(&mut pset).unwrap();
    for signer in signers {
        w1.sign(signer, &mut pset);
    }
    w1.send(&mut pset);
    w2.sync();

    assert_eq!(w1.balance(&asset), 9_000);
    assert_eq!(w2.balance(&asset), 1_000);

    // Spend mixed internal and external utxos
    let utxo = &w2.wollet.utxos().unwrap()[0];
    let external_utxo = w2.make_external(utxo);

    let mut pset = w1
        .tx_builder()
        .add_recipient(&w2.address(), 2_000, asset)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    w2.wollet.add_details(&mut pset).unwrap();
    for signer in signers {
        w1.sign(signer, &mut pset);
    }
    w1.send(&mut pset);
    w2.sync();

    assert_eq!(w1.balance(&asset), 8_000);
    assert_eq!(w2.balance(&asset), 2_000);

    // Spend mixed internal and external utxos (no change)
    let utxo = &w2.wollet.utxos().unwrap()[0];
    let external_utxo = w2.make_external(utxo);

    let mut pset = w1
        .tx_builder()
        .add_recipient(&w2.address(), 10_000, asset)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    w2.wollet.add_details(&mut pset).unwrap();
    for signer in signers {
        w1.sign(signer, &mut pset);
    }
    w1.send(&mut pset);
    w2.sync();

    assert_eq!(w1.balance(&asset), 0);
    assert_eq!(w2.balance(&asset), 10_000);
}

#[test]
fn test_unblinded_utxo() {
    // Receive unblinded utxo and spend it
    let server = setup();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut w = TestWollet::new(client, &desc);
    let signers = [&AnySigner::Software(signer)];

    let policy_asset = w.policy_asset();

    // Fund the wallet with an unblinded UTXO
    let satoshi = 100_000;
    w.fund_explicit(&server, satoshi, None, None);

    assert_eq!(w.balance(&policy_asset), 0);

    let external_utxo = w.wollet.explicit_utxos().unwrap()[0].clone();

    // Create tx sending the unblinded utxo
    let node_address = server.elementsd_getnewaddress();

    let mut pset = w
        .tx_builder()
        .add_lbtc_recipient(&node_address, 10_000)
        .unwrap()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    for signer in signers {
        w.sign(signer, &mut pset);
    }

    // Cannot get details
    let err = w.wollet.get_details(&pset).unwrap_err();
    assert_eq!(err.to_string(), "Input #0 is not blinded");

    w.send(&mut pset);

    // Received the change output
    assert!(w.balance(&policy_asset) > 0);

    // Fund the wallet with another unblinded UTXO
    w.fund_explicit(&server, satoshi, None, None);

    let explicit_utxos = w.wollet.explicit_utxos().unwrap();
    assert_eq!(explicit_utxos.len(), 1);
    let external_utxo = explicit_utxos.last().unwrap().clone();

    // Send all funds
    let mut pset = w
        .tx_builder()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .drain_lbtc_wallet()
        .drain_lbtc_to(node_address)
        .finish()
        .unwrap();

    // 1 blinded input, 1 unblinded input
    assert_eq!(pset.inputs().len(), 2);

    for signer in signers {
        w.sign(signer, &mut pset);
    }

    w.send(&mut pset);

    assert_eq!(w.balance(&policy_asset), 0);

    // 1 unblinded input, 1 blinded output: we can still blind the transaction
    w.fund_explicit(&server, satoshi, None, None);

    let explicit_utxos = w.wollet.explicit_utxos().unwrap();
    let external_utxo = explicit_utxos.last().unwrap().clone();

    // Send all funds
    let node_address = server.elementsd_getnewaddress();
    let mut pset = w
        .tx_builder()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .drain_lbtc_wallet()
        .drain_lbtc_to(node_address)
        .finish()
        .unwrap();

    for signer in signers {
        w.sign(signer, &mut pset);
    }

    w.send_outside_list(&mut pset);

    assert_eq!(w.balance(&policy_asset), 0);
}

#[test]
fn test_spend_blinded_utxo_with_custom_blinding_key() {
    let server = setup();
    let signer = generate_signer();
    let blinding_key = secp256k1::SecretKey::new(&mut rand::thread_rng());
    let desc = format!("ct(elip151,elwpkh({}/*))", signer.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut w = TestWollet::new(client, &desc);
    let policy_asset = w.policy_asset();
    let mut address_with_custom_blinding = w.address();
    address_with_custom_blinding.blinding_pubkey = Some(blinding_key.public_key(&EC));

    let amount = 100_000;
    let txid = server.elementsd_sendtoaddress(&address_with_custom_blinding, amount, None);
    server.elementsd_generate(1);

    w.wait_for_tx_outside_list(&txid);

    let mut utxos = w.wollet.unblind_utxos_with(blinding_key).unwrap();
    assert_eq!(utxos.len(), 1);
    let balance = w.balance(&policy_asset);
    assert_eq!(balance, 0, "unblindable utxos are considered");

    let external_utxo = utxos.pop().unwrap();

    // Sending the unblinded utxo to the wallet as correctly blinded output
    let mut pset = w
        .tx_builder()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .finish()
        .unwrap();

    signer.sign(&mut pset).unwrap();
    let _ = w.send(&mut pset);

    let balance = w.balance(&policy_asset);

    let details = w.wollet.get_details(&pset).unwrap();
    let fee = details.balance.fee;

    assert_eq!(balance, amount - fee);
}

#[cfg(feature = "elements_rpc")]
#[test]
fn test_elements_rpc() {
    let server = setup();
    assert_eq!(server.elementsd_height(), 101);
    let url = server.elements_rpc_url();
    let (user, pass) = server.elements_rpc_credentials();
    let network = ElementsNetwork::default_regtest();
    let elements_rpc_client =
        ElementsRpcClient::new_from_credentials(network, &url, &user, &pass).unwrap();
    assert_eq!(elements_rpc_client.height().unwrap(), 101);

    let auth = bitcoincore_rpc::Auth::UserPass(user, pass);
    let elements_rpc_client2 = ElementsRpcClient::new(network, &url, auth).unwrap();
    assert_eq!(elements_rpc_client2.height().unwrap(), 101);

    // Create wallet fund wallet
    let signer = generate_signer();
    let desc = format!("ct(elip151,elwpkh({}/*))", signer.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);
    let wd = wallet.wollet.wollet_descriptor();

    wallet.fund_btc(&server);
    let utxos = elements_rpc_client.confirmed_utxos(&wd, 20).unwrap();
    assert_eq!(utxos.len(), 0);

    // Confirm funds
    server.elementsd_generate(1);
    let utxos = elements_rpc_client.confirmed_utxos(&wd, 20).unwrap();
    assert_eq!(utxos.len(), 1);
}

#[cfg(feature = "esplora")]
#[test]
fn test_clients() {
    let server = setup_with_esplora();

    let electrum_url = ElectrumUrl::new(&server.electrs.electrum_url, false, false).unwrap();
    let electrum_client = ElectrumClient::new(&electrum_url).unwrap();

    let esplora_url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
    let esplora_client =
        clients::blocking::EsploraClient::new(&esplora_url, ElementsNetwork::default_regtest())
            .unwrap();

    assert_eq!(electrum_client.capabilities().len(), 0);
    assert_eq!(esplora_client.capabilities().len(), 0);

    let esplora_waterfalls_client = clients::blocking::EsploraClient::new_waterfalls(
        &esplora_url,
        ElementsNetwork::default_regtest(),
    )
    .unwrap();
    assert_eq!(esplora_waterfalls_client.capabilities().len(), 1);
}

fn wait_esplora_tx_update(client: &mut blocking::EsploraClient, wollet: &Wollet) -> Update {
    for _ in 0..50 {
        let update = client.full_scan(wollet).unwrap();
        if let Some(update) = update {
            if !update.only_tip() {
                return update;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("update didn't arrive");
}

#[cfg(feature = "esplora")]
#[test]
fn test_waterfalls_esplora() {
    // TODO: use TestWollet also for EsploraClient
    // FIXME: add launch_sync or similar to waterfalls

    init_logging();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let exe = std::env::var("ELEMENTSD_EXEC").unwrap();

    let test_env = rt.block_on(waterfalls::test_env::launch(exe));

    let url = format!("{}/blocks/tip/hash", test_env.base_url());
    let _r = reqwest::blocking::get(url).unwrap().text().unwrap();

    let mut client = clients::blocking::EsploraClient::new_waterfalls(
        test_env.base_url(),
        ElementsNetwork::default_regtest(),
    )
    .unwrap();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/<0;1>/*))", view_key, signer.xpub());
    let desc = WolletDescriptor::from_str(&desc).unwrap();
    let network = ElementsNetwork::default_regtest();
    let mut wollet = Wollet::without_persist(network, desc.clone()).unwrap();
    let update = client.full_scan(&wollet).unwrap().unwrap();
    wollet.apply_update(update).unwrap();

    let sats = 1_000;
    let address = wollet.address(None).unwrap();
    let _txid = test_env.send_to(address.address(), sats);

    let update = wait_esplora_tx_update(&mut client, &wollet);
    wollet.apply_update(update).unwrap();
    let balance = wollet.balance().unwrap();
    assert_eq!(sats, *balance.get(&network.policy_asset()).unwrap());

    let address = test_env.get_new_address(None);
    let mut pset = wollet
        .tx_builder()
        .drain_lbtc_wallet()
        .drain_lbtc_to(address.clone())
        .finish()
        .unwrap();

    let sigs = signer.sign(&mut pset).unwrap();
    assert!(sigs > 0);

    let tx = wollet.finalize(&mut pset).unwrap();
    let txid = client.broadcast(&tx).unwrap();

    let update = wait_esplora_tx_update(&mut client, &wollet);
    wollet.apply_update(update).unwrap();
    let balance = wollet.balance().unwrap();
    assert_eq!(0, *balance.get(&network.policy_asset()).unwrap());

    let elip151_desc = "ct(elip151,elwpkh(tpubDC3BrFCCjXq4jAceV8k6UACxDDJCFb1eb7R7BiKYUGZdNagEhNfJoYtUrRdci9JFs1meiGGModvmNm8PrqkrEjJ6mpt6gA1DRNU8vu7GqXH/<0;1>/*))";
    let elip151_desc = WolletDescriptor::from_str(elip151_desc).unwrap();
    let err = client
        .get_history_waterfalls(&elip151_desc, &wollet, 0)
        .unwrap_err();
    assert!(matches!(err, Error::UsingWaterfallsWithElip151));

    let history = client
        .get_scripts_history(&[&address.script_pubkey()])
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].len(), 1);
    assert_eq!(history[0][0].txid, txid);

    rt.block_on(test_env.shutdown());
}

#[cfg(feature = "esplora")]
#[test]
fn test_esplora_client() {
    let server = setup_with_esplora();
    let url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
    let client =
        clients::blocking::EsploraClient::new(&url, ElementsNetwork::default_regtest()).unwrap();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let signers = &[&AnySigner::Software(signer)];

    let wallet = TestWollet::new(client, &desc);
    roundtrip_inner(wallet, &server, signers);
}

#[test]
fn test_persistence_reload_after_only_tip() {
    let server = setup();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    server.elementsd_generate(1);
    wallet.wait_height(102);
    wallet.sync();

    TestWollet::check_persistence(wallet);
}

#[test]
fn test_non_standard_gap_limit() {
    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let wollet_desc = WolletDescriptor::from_str(&desc).unwrap();
    let mut client = test_client_electrum(&server.electrs.electrum_url);
    let network = ElementsNetwork::default_regtest();
    let satoshi = 1_000_000;

    let mut wollet_std_gap = Wollet::new(
        network,
        std::sync::Arc::new(NoPersist {}),
        wollet_desc.clone(),
    )
    .unwrap();
    let mut wollet_longer_gap =
        Wollet::new(network, std::sync::Arc::new(NoPersist {}), wollet_desc).unwrap();

    let i = Some(25);
    let address_after_gap_limit = wollet_std_gap.address(i).unwrap().address().clone();
    let address_check = wollet_longer_gap.address(i).unwrap().address().clone();
    assert_eq!(address_after_gap_limit, address_check);

    let txid = server.elementsd_sendtoaddress(&address_after_gap_limit, satoshi, None);
    server.elementsd_generate(1);

    // custom wait_for_tx using custom gap limit
    for i in 0..60 {
        full_scan_to_index_with_electrum_client(&mut wollet_longer_gap, 30, &mut client).unwrap();
        let tx_found = wollet_longer_gap
            .transactions()
            .unwrap()
            .iter()
            .any(|tx| tx.txid == txid);
        if tx_found {
            break;
        }
        if i == 59 {
            panic!("tx not found");
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let balance = wollet_longer_gap.balance().unwrap();
    assert_eq!(balance.get(&network.policy_asset()).unwrap(), &satoshi);

    // a normal sync on the wollet_long_gap should not lose the tx
    full_scan_with_electrum_client(&mut wollet_longer_gap, &mut client).unwrap();
    assert_eq!(balance.get(&network.policy_asset()).unwrap(), &satoshi);

    // a normal sync on the wollet_std_gap doesn't see the tx
    full_scan_with_electrum_client(&mut wollet_std_gap, &mut client).unwrap();
    let balance = wollet_std_gap.balance().unwrap();
    assert_eq!(balance.get(&network.policy_asset()).unwrap(), &0);
}

#[tokio::test]
#[cfg(feature = "esplora")]
async fn test_non_standard_gap_limit_esplora() {
    let server = setup_with_esplora();
    let url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
    let network = ElementsNetwork::default_regtest();
    let mut client = clients::asyncr::EsploraClient::new(network, &url);
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let wollet_desc = WolletDescriptor::from_str(&desc).unwrap();
    let satoshi = 1_000_000;

    let mut wollet = Wollet::new(network, std::sync::Arc::new(NoPersist {}), wollet_desc).unwrap();

    let i = Some(25);
    let address_after_gap_limit = wollet.address(i).unwrap().address().clone();

    let txid = server.elementsd_sendtoaddress(&address_after_gap_limit, satoshi, None);
    server.elementsd_generate(1);

    // custom wait_for_tx using custom gap limit
    for i in 0..60 {
        let update = client.full_scan_to_index(&wollet, 30).await.unwrap();
        if let Some(update) = update {
            wollet.apply_update(update).unwrap();
        }
        let tx_found = wollet
            .transactions()
            .unwrap()
            .iter()
            .any(|tx| tx.txid == txid);
        if tx_found {
            break;
        }
        if i == 59 {
            panic!("tx not found");
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let balance = wollet.balance().unwrap();
    assert_eq!(balance.get(&network.policy_asset()).unwrap(), &satoshi);
}

#[test]
#[cfg(feature = "esplora")]
fn test_non_standard_gap_limit_waterfalls_esplora() {
    // TODO: use TestWollet also for EsploraClient
    // FIXME: add launch_sync or similar to waterfalls

    init_logging();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let exe = std::env::var("ELEMENTSD_EXEC").unwrap();
    let test_env = rt.block_on(waterfalls::test_env::launch(exe));
    let url = format!("{}/blocks/tip/hash", test_env.base_url());
    let _r = reqwest::blocking::get(url).unwrap().text().unwrap();

    let mut client = clients::blocking::EsploraClient::new_waterfalls(
        test_env.base_url(),
        ElementsNetwork::default_regtest(),
    )
    .unwrap();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/<0;1>/*))", view_key, signer.xpub());
    let desc = WolletDescriptor::from_str(&desc).unwrap();
    let network = ElementsNetwork::default_regtest();
    let mut wollet = Wollet::without_persist(network, desc.clone()).unwrap();

    let i = Some(25);
    let address_after_gap_limit = wollet.address(i).unwrap().address().clone();

    let satoshi = 1_000_000;
    let txid = test_env.send_to(&address_after_gap_limit, satoshi);
    rt.block_on(test_env.node_generate(1));

    // custom wait_for_tx using custom gap limit
    for i in 0..60 {
        let update = client.full_scan_to_index(&wollet, 30).unwrap();
        if let Some(update) = update {
            wollet.apply_update(update).unwrap();
        }
        let tx_found = wollet
            .transactions()
            .unwrap()
            .iter()
            .any(|tx| tx.txid == txid);
        if tx_found {
            break;
        }
        if i == 59 {
            panic!("tx not found");
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    let balance = wollet.balance().unwrap();
    assert_eq!(balance.get(&network.policy_asset()).unwrap(), &satoshi);
}

#[test]
fn test_manual_coin_selection() {
    let server = setup();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut w = TestWollet::new(client, &desc);
    let node_address = server.elementsd_getnewaddress();

    let policy_asset = w.policy_asset();

    // Fund the wallet with 2 L-BTC UTXOs
    w.fund(&server, 100_000, None, None);
    w.fund(&server, 500_000, None, None);
    server.elementsd_generate(1);

    assert_eq!(w.balance(&policy_asset), 600_000);
    let utxos = w.wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 2);
    assert_eq!(
        utxos[0].unblinded.value, 500_000,
        "not sorted by biggest first"
    );
    assert_eq!(utxos[1].unblinded.value, 100_000);

    let err = w
        .tx_builder()
        .add_recipient(&node_address, 200_000, policy_asset)
        .unwrap()
        .set_wallet_utxos(vec![])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::InsufficientFunds { .. }));

    let err = w
        .tx_builder()
        .add_recipient(&node_address, 200_000, policy_asset)
        .unwrap()
        .set_wallet_utxos(vec![utxos[1].outpoint]) // not enough
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::InsufficientFunds { .. }));

    let mut pset = w
        .tx_builder()
        .add_recipient(&node_address, 200_000, policy_asset)
        .unwrap()
        .set_wallet_utxos(vec![utxos[0].outpoint])
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 1);
    assert_eq!(pset.outputs().len(), 3); // recipient + change + fee
    signer.sign(&mut pset).unwrap();
    let tx = w.wollet.finalize(&mut pset).unwrap();
    let tx = serialize(&tx);
    assert!(server.elementsd_testmempoolaccept(&tx.to_hex()));

    let mut pset = w
        .tx_builder()
        .add_recipient(&node_address, 200_000, policy_asset)
        .unwrap()
        .set_wallet_utxos(vec![utxos[0].outpoint, utxos[1].outpoint])
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 2);
    assert_eq!(pset.outputs().len(), 3); // recipient + change + fee
    signer.sign(&mut pset).unwrap();
    let tx = w.wollet.finalize(&mut pset).unwrap();
    let tx = serialize(&tx);
    assert!(server.elementsd_testmempoolaccept(&tx.to_hex()));

    let non_wallet_outpoint = OutPoint::new(txid_test_vector(), 0);
    let err = w
        .tx_builder()
        .add_recipient(&node_address, 200_000, policy_asset)
        .unwrap()
        .set_wallet_utxos(vec![non_wallet_outpoint])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::MissingWalletUtxo(_)));

    let signers = [&AnySigner::Software(signer)];
    let (asset, _) = w.issueasset(&signers, 10, 1, None, None);
    server.elementsd_generate(1);
    let utxos = w.wollet.utxos().unwrap();
    assert_eq!(utxos.len(), 3);
    let asset_utxo = &utxos[1];
    assert_eq!(asset_utxo.unblinded.value, 10);
    assert_eq!(asset_utxo.unblinded.asset, asset);

    let err = w
        .tx_builder()
        .add_recipient(&node_address, 200_000, policy_asset)
        .unwrap()
        .set_wallet_utxos(vec![asset_utxo.outpoint])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::ManualCoinSelectionOnlyLbtc));
    let err = w
        .tx_builder()
        .add_recipient(&node_address, 200_000, asset)
        .unwrap()
        .set_wallet_utxos(vec![utxos[0].outpoint])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::ManualCoinSelectionOnlyLbtc));
}

#[ignore = "This test connects to liquid testnet"]
#[test]
fn test_liquid_testnet() {
    let desc = "ct(slip77(ac53739ddde9fdf6bba3dbc51e989b09aa8c9cdce7b7d7eddd49cec86ddf71f7),elwpkh([93970d14/84'/1'/0']tpubDC3BrFCCjXq4jAceV8k6UACxDDJCFb1eb7R7BiKYUGZdNagEhNfJoYtUrRdci9JFs1meiGGModvmNm8PrqkrEjJ6mpt6gA1DRNU8vu7GqXH/<0;1>/*))#u0y4axgs";
    let wollet_desc = WolletDescriptor::from_str(desc).unwrap();
    let mut wollet = Wollet::new(
        ElementsNetwork::LiquidTestnet,
        std::sync::Arc::new(NoPersist {}),
        wollet_desc,
    )
    .unwrap();
    let url = "https://waterfalls.liquidwebwallet.org/liquidtestnet/api";
    let mut client = blocking::EsploraClient::new(url, ElementsNetwork::LiquidTestnet).unwrap();
    let update = client.full_scan(&wollet).unwrap().unwrap();
    let update_serialized = update.serialize().unwrap();
    std::fs::write("update.bin", update_serialized).unwrap();
    wollet.apply_update(update).unwrap();
}

#[test]
fn test_many_transactions() {
    let wollet = test_wollet::test_wollet_with_many_transactions();
    assert_eq!(wollet.transactions().unwrap().len(), 63);
    let balance = wollet.balance().unwrap();
    assert_eq!(format!("{:?}", balance), "{144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49: 1093721, 0cf33929dd6f87ae71d3c500aa056f6dbd027bcb3051b1dae6fe67750fbccd76: 5, 39ee0a62f96c5b5bd28266769ab4d7df28777ed2988f3818fffe48c4c5ba0f84: 1, 38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5: 9876, bf83e69c997b3336b731d1207e1dd8967dd089edfe55f96586c858f3a6da76bf: 1, 91618f01b2ec10c6cb6d03ea4fde9d765e30c23b8585522d247972a31c5435d6: 210}");
}

#[test]
fn test_update_v2_after_old_updates() {
    let mut wollet = test_wollet::test_wollet_with_many_transactions();
    assert_eq!(wollet.transactions().unwrap().len(), 63);
    let update = Update::deserialize(&update_v2_test_vector_after_many_transactions()).unwrap();
    assert_eq!(update.version, 2);
    assert!(!update.new_txs.txs.is_empty());
    wollet.apply_update(update).unwrap();
    assert_eq!(wollet.transactions().unwrap().len(), 64);
}

fn liquidex<C: BlockchainBackend>(
    wallet_maker: &mut TestWollet<C>,
    signer_maker: &AnySigner,
    wallet_taker: &mut TestWollet<C>,
    signer_taker: &AnySigner,
    utxo_send: OutPoint,
    sats_recv: u64,
    asset_recv: elements::AssetId,
) {
    // LiquiDEX make
    let addr = wallet_maker.address_result(None).address().clone();
    let mut pset = wallet_maker
        .tx_builder()
        .liquidex_make(utxo_send, &addr, sats_recv, asset_recv)
        .unwrap()
        .finish()
        .unwrap();

    let details = wallet_maker.wollet.get_details(&pset).unwrap();
    assert_eq!(details.balance.fee, 0);
    let asset_send = pset.inputs()[0].asset.unwrap();
    let sats_send = pset.inputs()[0].amount.unwrap();
    let from_details_send = *details.balance.balances.get(&asset_send).unwrap();
    let from_details_recv = *details.balance.balances.get(&asset_recv).unwrap();
    assert_eq!(from_details_send, -(sats_send as i64));
    assert_eq!(from_details_recv, sats_recv as i64);

    wallet_maker.sign(signer_maker, &mut pset);
    let proposal = LiquidexProposal::from_pset(&pset).unwrap();

    let txid = proposal.needed_tx().unwrap();
    assert_eq!(txid, utxo_send.txid);
    let tx = wallet_maker.wollet.transaction(&txid).unwrap().unwrap().tx;
    let proposal = proposal.validate(tx).unwrap();

    // Extract validated assets and amounts from the proposal
    let AssetAmount {
        amount: maker_input_sats,
        asset: maker_input_asset,
    } = proposal.input();
    assert_eq!(maker_input_sats, pset.inputs()[0].amount.unwrap());
    assert_eq!(maker_input_asset, pset.inputs()[0].asset.unwrap());
    let AssetAmount {
        amount: maker_output_sats,
        asset: maker_output_asset,
    } = proposal.output();
    assert_eq!(maker_output_sats, sats_recv);
    assert_eq!(maker_output_asset, asset_recv);

    // LiquiDEX take
    let mut pset = wallet_taker
        .tx_builder()
        .liquidex_take(vec![proposal])
        .unwrap()
        .finish()
        .unwrap();

    let details = wallet_taker.wollet.get_details(&pset).unwrap();
    let fee = details.balance.fee as i64;
    assert!(fee > 0);
    // "send" and "recv" are from the maker perspective
    let mut from_details_send = *details.balance.balances.get(&asset_send).unwrap();
    let mut from_details_recv = *details.balance.balances.get(&asset_recv).unwrap();
    let policy_asset = wallet_taker.policy_asset();
    if asset_send == policy_asset {
        from_details_send += fee;
    }
    if asset_recv == policy_asset {
        from_details_recv += fee;
    }
    assert_eq!(from_details_send, sats_send as i64);
    assert_eq!(from_details_recv, -(sats_recv as i64));

    wallet_taker.sign(signer_taker, &mut pset);
    let _txid = wallet_taker.send(&mut pset);
    wait_tx_update(wallet_maker);
}

#[test]
fn test_liquidex() {
    let server = setup();

    // Alice
    let signer_a = generate_signer();
    let view_key = generate_view_key();
    let desc_a = format!("ct({},elwpkh({}/*))", view_key, signer_a.xpub());
    let sa = AnySigner::Software(signer_a);
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wa = TestWollet::new(client, &desc_a);

    // Bob
    let signer_b = generate_signer();
    let view_key = generate_view_key();
    let desc_b = format!("ct({},elwpkh({}/*))", view_key, signer_b.xpub());
    let sb = AnySigner::Software(signer_b);
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wb = TestWollet::new(client, &desc_b);

    wa.fund_btc(&server);
    wb.fund_btc(&server);

    let (asset_1, _) = wa.issueasset(&[&sa], 10, 1, None, None);
    let (asset_2, _) = wb.issueasset(&[&sb], 10, 1, None, None);

    assert_eq!(wa.balance(&asset_1), 10);
    assert_eq!(wa.balance(&asset_2), 0);
    assert_eq!(wb.balance(&asset_1), 0);
    assert_eq!(wb.balance(&asset_2), 10);
    let policy_asset = wa.policy_asset();

    // Maker: A, sends LBTC, receives 1 of asset_2
    let utxo = wa
        .wollet
        .utxos()
        .unwrap()
        .into_iter()
        .find(|u| u.unblinded.asset == policy_asset)
        .unwrap()
        .outpoint;
    liquidex(&mut wa, &sa, &mut wb, &sb, utxo, 1, asset_2);
    assert_eq!(wa.balance(&asset_1), 10);
    assert_eq!(wa.balance(&asset_2), 1);
    assert_eq!(wa.balance(&policy_asset), 0);
    assert_eq!(wb.balance(&asset_1), 0);
    assert_eq!(wb.balance(&asset_2), 9);

    // Maker: A, sends asset_2, receives LBTC
    let utxo = wa
        .wollet
        .utxos()
        .unwrap()
        .into_iter()
        .find(|u| u.unblinded.asset == asset_2)
        .unwrap()
        .outpoint;
    liquidex(&mut wa, &sa, &mut wb, &sb, utxo, 10_000, policy_asset);
    assert_eq!(wa.balance(&asset_1), 10);
    assert_eq!(wa.balance(&asset_2), 0);
    assert_eq!(wa.balance(&policy_asset), 10_000);
    assert_eq!(wb.balance(&asset_1), 0);
    assert_eq!(wb.balance(&asset_2), 10);

    // Maker: A, sends asset_1, receives asset_2
    let utxo = wa
        .wollet
        .utxos()
        .unwrap()
        .into_iter()
        .find(|u| u.unblinded.asset == asset_1)
        .unwrap()
        .outpoint;
    liquidex(&mut wa, &sa, &mut wb, &sb, utxo, 1, asset_2);
    assert_eq!(wa.balance(&asset_1), 0);
    assert_eq!(wa.balance(&asset_2), 1);
    assert_eq!(wa.balance(&policy_asset), 10_000);
    assert_eq!(wb.balance(&asset_1), 10);
    assert_eq!(wb.balance(&asset_2), 9);

    // TODO: check fees
}

#[test]
fn test_no_wildcard() {
    let server = setup_with_esplora();

    let slip77_key = generate_slip77();
    let signer = generate_signer();
    let xpub = signer.xpub();
    let desc = format!("ct(slip77({}),elwpkh({}))", slip77_key, xpub);

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    // Receive
    wallet.fund_btc(&server);

    // Send
    let balance_before = wallet.balance_btc();
    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&wallet.address(), 10_000)
        .unwrap()
        .finish()
        .unwrap();
    pset = pset_rt(&pset);

    let details = wallet.wollet.get_details(&pset).unwrap();
    let fee = details.balance.fee as i64;
    assert!(fee > 0);
    // TODO: fix balance computation for this case, then use send_btc in this test
    assert!(!details
        .balance
        .balances
        .contains_key(&wallet.policy_asset()));

    wallet.sign(&signer, &mut pset);
    let txid = wallet.send(&mut pset);
    let balance_after = wallet.balance_btc();
    assert!(balance_before > balance_after);
    let tx = wallet.get_tx(&txid);
    assert_eq!(&tx.type_, "outgoing");

    let txs = wallet.wollet.transactions().unwrap();
    assert_eq!(txs.len(), 2);

    // Use esplora client
    let network = ElementsNetwork::default_regtest();
    let mut esplora_wollet = Wollet::new(
        network,
        std::sync::Arc::new(NoPersist {}),
        desc.parse().unwrap(),
    )
    .unwrap();

    let esplora_url = format!("http://{}", server.electrs.esplora_url.as_ref().unwrap());
    let mut esplora_client = clients::blocking::EsploraClient::new(&esplora_url, network).unwrap();

    let update = esplora_client.full_scan(&esplora_wollet).unwrap();
    if let Some(update) = update {
        esplora_wollet.apply_update(update).unwrap();
    }

    let esplora_txs = esplora_wollet.transactions().unwrap();
    assert_eq!(esplora_txs.len(), 2);

    // TODO: waterfalls support
    /*
    // Use waterfalls client
    let rt = tokio::runtime::Runtime::new().unwrap();
    let exe = std::env::var("ELEMENTSD_EXEC").unwrap();
    let test_env = rt.block_on(waterfalls::test_env::launch(exe));

    let url = format!("{}/blocks/tip/hash", test_env.base_url());
    let _r = reqwest::blocking::get(url).unwrap().text().unwrap();

    let mut waterfalls_client = clients::blocking::EsploraClient::new_waterfalls(
        test_env.base_url(),
        network,
    )
    .unwrap();

    let mut waterfalls_wollet = Wollet::new(
        network,
        std::sync::Arc::new(NoPersist {}),
        desc.parse().unwrap(),
    )
    .unwrap();

    let update = waterfalls_client.full_scan(&waterfalls_wollet).unwrap();
    if let Some(update) = update {
        waterfalls_wollet.apply_update(update).unwrap();
    }

    let waterfalls_txs = waterfalls_wollet.transactions().unwrap();
    assert_eq!(waterfalls_txs.len(), 2);
     * */
}

#[test]
fn test_sh_multi() {
    //let server = setup_with_esplora();
    let server = setup();

    let slip77_key = generate_slip77();
    let signer1 = generate_signer();
    let signer2 = generate_signer();
    let xpub1 = signer1.xpub();
    let xpub2 = signer2.xpub();
    let desc = format!(
        "ct(slip77({}),elsh(multi(1,{}/*,{}/*)))",
        slip77_key, xpub1, xpub2
    );

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    // Receive
    wallet.fund_btc(&server);

    // Send
    let balance_before = wallet.balance_btc();
    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&wallet.address(), 10_000)
        .unwrap()
        .finish()
        .unwrap();
    pset = pset_rt(&pset);

    let details = wallet.wollet.get_details(&pset).unwrap();
    let fee = details.balance.fee as i64;
    assert!(fee > 0);
    // TODO: fee rate estimation is off, fix it and use send_btc in this test
    assert!(compute_fee_rate(&pset) > 100.0);

    wallet.sign(&signer1, &mut pset);
    let txid = wallet.send(&mut pset);
    let balance_after = wallet.balance_btc();
    assert!(balance_before > balance_after);
    let tx = wallet.get_tx(&txid);
    assert_eq!(&tx.type_, "outgoing");

    let txs = wallet.wollet.transactions().unwrap();
    assert_eq!(txs.len(), 2);
}

#[test]
fn test_singlekey() {
    let server = setup_with_esplora();
    let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
    let sk_a = secp256k1::SecretKey::from_str(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .unwrap();
    let sk_b = secp256k1::SecretKey::from_str(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )
    .unwrap();
    let sk_c = secp256k1::SecretKey::from_str(
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    )
    .unwrap();
    let pk_a = sk_a.public_key(&EC);
    let pk_b = sk_b.public_key(&EC);
    let pk_c = sk_c.public_key(&EC);
    let desc = format!("ct({},elsh(multi(2,{},{},{})))", view_key, pk_a, pk_b, pk_c);
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    wallet.fund_btc(&server);
    let balance_before = wallet.balance_btc();

    // Send some L-BTC to another address
    let node_addr = server.elementsd_getnewaddress();
    let satoshi = 5000;

    // Create tx
    let mut pset = wallet
        .tx_builder()
        .add_lbtc_recipient(&node_addr, satoshi)
        .unwrap()
        .finish()
        .unwrap();

    sign_with_seckey(sk_a, &mut pset).unwrap();
    sign_with_seckey(sk_b, &mut pset).unwrap();

    // Finalize and send the PSET
    wallet.send(&mut pset);

    let balance_after = wallet.balance_btc();

    assert!(balance_before > balance_after);
}

#[test]
fn test_issuance_amount_limits() {
    let server = setup();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);
    wallet.fund_btc(&server);

    // Let's test an issuance of 21M*10^8,
    let amount_21m = 21_000_000 * 100_000_000;

    let mut pset = wallet
        .tx_builder()
        .issue_asset(amount_21m, None, 1, None, None)
        .unwrap()
        .finish()
        .unwrap();
    wallet.sign(&AnySigner::Software(signer.clone()), &mut pset);
    let (asset, _) = pset.inputs()[0].issuance_ids();
    wallet.send(&mut pset);

    // Let's test an issuance of 21M*10^8+1, which should be valid but the node rejects it.
    let amount_over_btc_max = 21_000_000 * 100_000_000 + 1;

    let issue_error = wallet
        .tx_builder()
        .issue_asset(amount_over_btc_max, None, 1, None, None)
        .unwrap_err();

    assert_eq!(
        issue_error.to_string(),
        "Issuance amount greater than 21M*10^8 are not allowed"
    );

    // Before introducing IssuanceAmountGreaterThanBtcMax error this was testing the effectively the node rejects it.
    // With the error in place this is hard to test.

    // wallet.sign(&AnySigner::Software(signer.clone()), &mut pset);
    // let tx = wallet.wollet.finalize(&mut pset).unwrap();
    // let tx_hex = elements::encode::serialize(&tx).to_hex();

    // // The node rejects more than 21M BTC issuance.
    // assert!(!server.elementsd_testmempoolaccept(&tx_hex));

    let amount = 21_000_000 * 100_000_000;
    let mut pset = wallet
        .tx_builder()
        .reissue_asset(asset, amount, None, None)
        .unwrap()
        .finish()
        .unwrap();
    wallet.sign(&AnySigner::Software(signer.clone()), &mut pset);
    wallet.send(&mut pset);

    let reissue_error = wallet
        .tx_builder()
        .reissue_asset(asset, amount_over_btc_max, None, None)
        .unwrap_err();

    assert_eq!(
        reissue_error.to_string(),
        "Issuance amount greater than 21M*10^8 are not allowed"
    );
}

#[test]
fn test_non_std_legacy_multisig() {
    let server = setup_with_esplora();

    // Receiver wallet
    let recv_signer = generate_signer();
    let recv_xpub = recv_signer.xpub();
    let recv_desc = format!("ct(elip151,elwpkh({}/*))", recv_xpub);
    let recv_client = test_client_electrum(&server.electrs.electrum_url);
    let mut recv_wallet = TestWollet::new(recv_client, &recv_desc);
    let recv_addr = recv_wallet.address();
    assert_eq!(recv_wallet.balance_btc(), 0);

    // P2SH 2of3 with 3 single keys blinded in a non standard way

    // 3 single keys
    let wif_aa = "cTJTN1hGHqucsgqmYVbhU3g4eU9g5HzE1sxuSY32M1xap1K4sYHF";
    let wif_bb = "cTsdXxTC346tsb7HaddDzC5dTqAT8XCsdJsacS4N3ak2mCGGZcN5";
    let wif_cc = "cUSohuD7nGJAsVNocmekWLVCHCBEBkRXEjnFnL5hk9XUiPBCLR4d";
    let sk_a = bitcoin::PrivateKey::from_wif(wif_aa).unwrap().inner;
    let sk_b = bitcoin::PrivateKey::from_wif(wif_bb).unwrap().inner;
    let sk_c = bitcoin::PrivateKey::from_wif(wif_cc).unwrap().inner;
    let pk_a = sk_a.public_key(&EC);
    let pk_b = sk_b.public_key(&EC);
    let pk_c = sk_c.public_key(&EC);

    // A temporary descriptor blinding key
    let view_key = "1111111111111111111111111111111111111111111111111111111111111111";
    // P2SH 2of3 with 3 single pubkeys
    let desc = format!("ct({},elsh(multi(2,{},{},{})))", view_key, pk_a, pk_b, pk_c);

    // Create the wallet
    let client = test_client_electrum(&server.electrs.electrum_url);
    let mut wallet = TestWollet::new(client, &desc);

    // Get an address
    let mut addr = wallet.address();
    // But use another blinding key
    // `elements-cli dumpblindingkey $ADDR` returns 64 hex chars
    let blinding_privkey = secp256k1::SecretKey::from_str(
        "7777777777777777777777777777777777777777777777777777777777777777",
    )
    .unwrap();
    addr.blinding_pubkey = Some(blinding_privkey.public_key(&EC));

    // Fund the address with an asset
    let satoshi = 10_000;
    let asset = server.elementsd_issueasset(satoshi);
    let txid = server.elementsd_sendtoaddress(&addr, satoshi, Some(asset));
    wallet.wait_for_tx_outside_list(&txid);

    // Get external utxo
    let mut utxos = wallet.wollet.unblind_utxos_with(blinding_privkey).unwrap();
    assert_eq!(utxos.len(), 1);
    let external_utxo = utxos.pop().unwrap();

    // Fund with some btc for the fees
    wallet.fund_btc(&server);

    // Create spending tx
    let mut pset = wallet
        .tx_builder()
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .add_recipient(&recv_addr, satoshi, asset)
        .unwrap()
        .drain_lbtc_wallet()
        .drain_lbtc_to(recv_addr)
        .finish()
        .unwrap();

    sign_with_seckey(sk_a, &mut pset).unwrap();
    sign_with_seckey(sk_b, &mut pset).unwrap();
    wallet.send(&mut pset);
    recv_wallet.sync();

    // Check receiver balance
    assert_eq!(recv_wallet.balance(&asset), 10_000);
    assert!(recv_wallet.balance_btc() > 0);
    assert_eq!(wallet.balance_btc(), 0);
}
