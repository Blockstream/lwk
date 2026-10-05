use std::str::FromStr;

use crate::test_wollet::*;
use elements::encode::serialize;
use elements::hex::ToHex;
use elements::OutPoint;
use lwk_common::{Network, Signer};
use lwk_signer::{AnySigner, SwSigner};
use lwk_test_util::*;
use lwk_wollet::{blocking::BlockchainBackend, WolletBuilder, WolletDescriptor};
use lwk_wollet::{Contract, Error, IssuanceRequest, ReissuanceRequest, WalletTx, WalletTxOut};

#[test]
fn test_assets_owned() {
    let network = Network::default_regtest();
    let policy_asset = *network.policy_asset();
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let client = test_client_electrum(&env.electrum_url());

    let signer = generate_signer();
    let desc = signer.wpkh_slip77_descriptor().unwrap();
    let mut wallet = TestWollet::new(client, &desc);

    wallet.fund_btc(&env);

    // Initially only policy_asset is owned
    let owned = wallet.wollet.assets_owned().unwrap();
    assert_eq!(owned.len(), 1);
    assert!(owned.contains(&policy_asset));

    // Confidential assets
    let asset_id = wallet.fund_asset(&env);

    let owned = wallet.wollet.assets_owned().unwrap();
    assert!(owned.contains(&asset_id));

    // Unconfidential assets
    let asset_id_explicit = env.elementsd_issueasset(100_000);
    env.elementsd_generate(1);
    let _ = wallet.fund_explicit(&env, 10_000, None, Some(asset_id_explicit));

    let owned = wallet.wollet.assets_owned().unwrap();
    assert!(owned.contains(&asset_id_explicit));

    // Spend all assets
    let external_address = env.elementsd_getnewaddress();
    let explicit_utxos = wallet.wollet.explicit_utxos().unwrap();
    let mut pset = wallet
        .tx_builder()
        .add_recipient(&external_address, 10_000, asset_id)
        .unwrap()
        .drain_lbtc_to(&external_address)
        .unwrap()
        .add_external_utxos(explicit_utxos)
        .unwrap()
        .finish()
        .unwrap();

    signer.sign(&mut pset).unwrap();
    wallet.send(&mut pset);

    // assets_owned should still include this assets even after it was fully spent
    let owned = wallet.wollet.assets_owned().unwrap();
    assert!(owned.contains(&asset_id));
    assert!(owned.contains(&asset_id_explicit));
    assert!(owned.contains(&policy_asset));
}

#[test]
fn test_issuance_amount_limits() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());

    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc);
    wallet.fund_btc(&env);

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
    assert_eq!(wallet.balance(&asset), amount_21m);

    // Let's test an issuance of 21M*10^8+1, nodes now accepts it
    let amount_over_btc_max = 21_000_000 * 100_000_000 + 1;

    let mut pset = wallet
        .tx_builder()
        .issue_asset(amount_over_btc_max, None, 1, None, None)
        .unwrap()
        .finish()
        .unwrap();

    wallet.sign(&AnySigner::Software(signer.clone()), &mut pset);
    let (asset, _) = pset.inputs()[0].issuance_ids();
    wallet.send(&mut pset);

    // We can also reissuance more than 21M
    let mut pset = wallet
        .tx_builder()
        .reissue_asset(asset, amount_over_btc_max, None, None)
        .unwrap()
        .finish()
        .unwrap();
    wallet.sign(&AnySigner::Software(signer.clone()), &mut pset);
    wallet.send(&mut pset);

    assert_eq!(wallet.balance(&asset), 2 * amount_over_btc_max);
}

#[test]
fn test_issue_asset() -> Result<(), Box<dyn std::error::Error>> {
    use bip39::Mnemonic;
    // Test based on Python bindings test issue_asset.py
    let network = Network::default_regtest();
    let policy_asset = *network.policy_asset();
    let env = TestEnvBuilder::from_env().with_electrum().build();

    // ANCHOR: test_issue_asset

    let mut client = test_client_electrum(&env.electrum_url());

    // Create wallet
    let mnemonic = Mnemonic::generate(12)?;

    let signer = SwSigner::new_with_network(&mnemonic.to_string(), network)?;
    let desc = signer.wpkh_slip77_descriptor()?;

    let mut wollet = WolletBuilder::new(network, WolletDescriptor::from_str(&desc)?).build()?;
    let wollet_address = wollet.address(None)?;
    assert!(wollet_address.index() == 0); // ANCHOR: ignore

    let funded_satoshi = 100_000; // ANCHOR: ignore
    let txid =
        env.elementsd_sendtoaddress(wollet_address.address(), funded_satoshi, Some(policy_asset)); // ANCHOR: ignore
    wait_for_tx(&mut wollet, &mut client, &txid); // ANCHOR: ignore
    assert!(*wollet.balance()?.get(&policy_asset).unwrap_or(&0) == funded_satoshi); // ANCHOR: ignore

    // ANCHOR: contract
    let contract_str = "{\"entity\":{\"domain\":\"ciao.it\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"name\",\"precision\":8,\"ticker\":\"TTT\",\"version\":0}";
    let contract = Contract::from_str(contract_str)?;
    // assert!(contract.to_string() == ("{\"entity\":{\"domain\":\"ciao.it\"},\"issuer_pubkey\":\"0337cceec0beea0232ebe14cba0197a9fbd45fcf2ec946749de920e71434c2b904\",\"name\":\"name\",\"precision\":8,\"ticker\":\"TTT\",\"version\":0}")); // ANCHOR: ignore
    // ANCHOR_END: contract

    // ANCHOR: issue_asset
    // Issue asset
    let issued_asset = 10_000;
    let reissuance_tokens = 1;

    // Create a transaction builder and the issuance transaction
    let builder = wollet.tx_builder();
    //  isue asset
    let mut pset = builder
        .issue_asset(
            issued_asset,
            None, // None -> a wallet from the address is used
            reissuance_tokens,
            None, // None -> a wallet from the address is used
            Some(contract.clone()),
        )?
        .finish()?;

    // Sign the transaction and finalize it
    let signatures_added = signer.sign(&mut pset).expect("signing failed");
    assert!(signatures_added == 1); // ANCHOR: ignore
    let _ = wollet.finalize(&mut pset)?;
    let tx = pset.extract_tx()?;

    // Broadcast the transaction
    let txid = client.broadcast(&tx)?;
    // ANCHOR_END: issue_asset

    // ANCHOR: issuance_ids
    let asset_id = pset.inputs()[0].issuance_ids().0;
    let token_id = pset.inputs()[0].issuance_ids().1;
    // ANCHOR_END: issuance_ids
    // ANCHOR_END: test_issue_asset
    //let txin = tx.inputs()[0];

    wait_for_tx(&mut wollet, &mut client, &txid);

    assert!(*wollet.balance()?.get(&asset_id).unwrap_or(&0) == issued_asset);
    assert!(*wollet.balance()?.get(&token_id).unwrap_or(&0) == reissuance_tokens);

    // ANCHOR: reissue_asset
    let reissue_asset = 100;
    let asset_receiver = None; // Send the asset to the wollet creating the PSET
    let issuance_tx = None; // issunce transaction is present in the same wallet
    let builder = wollet.tx_builder();
    let mut pset = builder
        .reissue_asset(asset_id, reissue_asset, asset_receiver, issuance_tx)?
        .finish()?;
    let signatures_added = signer.sign(&mut pset).unwrap();
    assert!(signatures_added == 2); // ANCHOR: ignore
    let _ = wollet.finalize(&mut pset).unwrap();
    let tx = pset.extract_tx().unwrap();
    let txid = client.broadcast(&tx).unwrap();
    // ANCHOR_END: reissue_asset

    wait_for_tx(&mut wollet, &mut client, &txid);

    assert!(
        *wollet.balance().unwrap().get(&asset_id).unwrap_or(&0) == issued_asset + reissue_asset
    );

    // ANCHOR: burn_asset
    let burn_asset = 50;
    let builder = wollet.tx_builder();
    let mut pset = builder.add_burn(burn_asset, asset_id)?.finish()?;
    let signatures_added = signer.sign(&mut pset)?;
    assert!(signatures_added == 2); // ANCHOR: ignore
    let _ = wollet.finalize(&mut pset)?;
    let tx = pset.extract_tx()?;
    let txid = client.broadcast(&tx)?;
    // ANCHOR_END: burn_asset

    wait_for_tx(&mut wollet, &mut client, &txid);

    assert!(
        *wollet.balance()?.get(&asset_id).unwrap_or(&0)
            == issued_asset + reissue_asset - burn_asset
    );

    Ok(())
}

#[test]
fn test_multiple_issuances() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);

    let policy_asset = w.policy_asset();

    // Fund the wallet with 2 L-BTC UTXOs
    w.fund(&env, 100_000, None, None);
    w.fund(&env, 500_000, None, None);
    env.elementsd_generate(1);

    let utxos: Vec<_> = w
        .wollet
        .utxos()
        .unwrap()
        .iter()
        .map(|u| u.outpoint)
        .collect();
    assert_eq!(utxos.len(), 2);

    let dummy_asset = elements::AssetId::from_slice(&[1u8; 32]).unwrap();
    let dummy_outpoint = OutPoint::null();

    // Issuance and reissuance are mutually exclusive, in both orders
    let err = w
        .tx_builder()
        .issue_asset(10, None, 1, None, None)
        .unwrap()
        .reissue_asset(dummy_asset, 5, None, None)
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceReissuanceMutuallyExclusive));

    let err = w
        .tx_builder()
        .reissue_asset(dummy_asset, 5, None, None)
        .unwrap()
        .issue_asset(10, None, 1, None, None)
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceReissuanceMutuallyExclusive));

    let err = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(dummy_outpoint))
        .unwrap()
        .reissue_asset(dummy_asset, 5, None, None)
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceReissuanceMutuallyExclusive));

    let err = w
        .tx_builder()
        .reissue_asset(dummy_asset, 5, None, None)
        .unwrap()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(dummy_outpoint))
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceReissuanceMutuallyExclusive));

    // Pinned and non-pinned issuances can't be mixed, in both orders
    let err = w
        .tx_builder()
        .issue_asset(10, None, 1, None, None)
        .unwrap()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(dummy_outpoint))
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceModesMixing));

    let err = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(dummy_outpoint))
        .unwrap()
        .issue_asset(10, None, 1, None, None)
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceModesMixing));

    // Repeated calls of the same kind accumulate instead of erroring
    w.tx_builder()
        .issue_asset(10, None, 1, None, None)
        .unwrap()
        .issue_asset(10, None, 1, None, None)
        .unwrap();
    w.tx_builder()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(dummy_outpoint))
        .unwrap()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(OutPoint::new(
            <elements::Txid as elements::hashes::Hash>::all_zeros(),
            1,
        )))
        .unwrap();

    // More issuances than transaction inputs
    let err = w
        .tx_builder()
        .issue_asset(10, None, 1, None, None)
        .unwrap()
        .issue_asset(20, None, 2, None, None)
        .unwrap()
        .set_wallet_utxos(vec![utxos[0]])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceInputCountMismatch));

    // Two issuances in the same transaction, assigned sequentially to the inputs
    let mut pset = w
        .tx_builder()
        .issue_asset(10, None, 1, None, None)
        .unwrap()
        .issue_asset(20, None, 2, None, None)
        .unwrap()
        .set_wallet_utxos(utxos)
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 2);
    assert_eq!(pset.inputs()[0].issuance_value_amount, Some(10));
    assert_eq!(pset.inputs()[0].issuance_inflation_keys, Some(1));
    assert_eq!(pset.inputs()[1].issuance_value_amount, Some(20));
    assert_eq!(pset.inputs()[1].issuance_inflation_keys, Some(2));
    let (asset0, token0) = pset.inputs()[0].issuance_ids();
    let (asset1, token1) = pset.inputs()[1].issuance_ids();
    assert_ne!(asset0, asset1);

    let details = w.wollet.get_details(&pset).unwrap();
    assert_eq!(n_issuances(&details), 2);
    assert_eq!(n_reissuances(&details), 0);

    signer.sign(&mut pset).unwrap();
    w.send(&mut pset);
    assert_eq!(w.balance(&asset0), 10);
    assert_eq!(w.balance(&token0), 1);
    assert_eq!(w.balance(&asset1), 20);
    assert_eq!(w.balance(&token1), 2);
    env.elementsd_generate(1);

    // Fund again so the wallet has 2 L-BTC UTXOs for the pinned issuance cases
    w.fund(&env, 100_000, None, None);
    env.elementsd_generate(1);
    let lbtc: Vec<_> = w
        .wollet
        .utxos()
        .unwrap()
        .iter()
        .filter(|u| u.unblinded.asset == policy_asset)
        .map(|u| u.outpoint)
        .collect();
    assert_eq!(lbtc.len(), 2);

    // Pinning an issuance requires a manual inputs order
    let err = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(lbtc[0]))
        .unwrap()
        .set_wallet_utxos(vec![lbtc[0]])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::IssuancePinRequiresInputsOrder));

    // Pinning to an outpoint not present in the inputs order
    let err = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(lbtc[0]))
        .unwrap()
        .set_wallet_utxos(vec![lbtc[1]])
        .set_inputs_order(vec![lbtc[1]])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceOutpointNotInInputsOrder(o) if o == lbtc[0]));

    // Two issuances pinned to the same outpoint
    let err = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(lbtc[0]))
        .unwrap()
        .add_issuance(IssuanceRequest::new(20, 2).pin_input(lbtc[0]))
        .unwrap()
        .set_wallet_utxos(vec![lbtc[0], lbtc[1]])
        .set_inputs_order(vec![lbtc[0], lbtc[1]])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::DuplicatedOutpoint(o, _) if o == lbtc[0]));

    // More pinned issuances than inputs
    let err = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(10, 1).pin_input(lbtc[0]))
        .unwrap()
        .add_issuance(IssuanceRequest::new(20, 2).pin_input(lbtc[1]))
        .unwrap()
        .set_wallet_utxos(vec![lbtc[0]])
        .set_inputs_order(vec![lbtc[0]])
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::IssuanceInputCountMismatch));

    // Pin an issuance to the input placed second, leaving the first input without issuance
    let mut pset = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(30, 3).pin_input(lbtc[1]))
        .unwrap()
        .set_wallet_utxos(vec![lbtc[0], lbtc[1]])
        .set_inputs_order(vec![lbtc[0], lbtc[1]])
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 2);
    let in1 = OutPoint::new(
        pset.inputs()[1].previous_txid,
        pset.inputs()[1].previous_output_index,
    );
    assert_eq!(in1, lbtc[1]);
    assert_eq!(pset.inputs()[0].issuance_value_amount, None);
    assert_eq!(pset.inputs()[1].issuance_value_amount, Some(30));
    assert_eq!(pset.inputs()[1].issuance_inflation_keys, Some(3));
    let (asset2, token2) = pset.inputs()[1].issuance_ids();

    signer.sign(&mut pset).unwrap();
    w.send(&mut pset);
    assert_eq!(w.balance(&asset2), 30);
    assert_eq!(w.balance(&token2), 3);
    env.elementsd_generate(1);

    // An issuance can also be pinned to an external input
    let signer2 = generate_signer();
    let view_key2 = generate_view_key();
    let desc2 = format!("ct({},elwpkh({}/*))", view_key2, signer2.xpub());
    let client2 = test_client_electrum(&env.electrum_url());
    let mut w2 = TestWollet::new(client2, &desc2);
    w2.fund(&env, 200_000, None, None);
    env.elementsd_generate(1);
    let external_utxo = w2.make_external(&w2.wollet.utxos().unwrap()[0]);
    let external_outpoint = external_utxo.outpoint;

    let lbtc_utxo = w
        .wollet
        .utxos()
        .unwrap()
        .iter()
        .find(|u| u.unblinded.asset == policy_asset)
        .unwrap()
        .outpoint;

    let mut pset = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(40, 4).pin_input(external_outpoint))
        .unwrap()
        .set_wallet_utxos(vec![lbtc_utxo])
        .add_external_utxos(vec![external_utxo])
        .unwrap()
        .set_inputs_order(vec![lbtc_utxo, external_outpoint])
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 2);
    assert_eq!(pset.inputs()[0].issuance_value_amount, None);
    assert_eq!(pset.inputs()[1].issuance_value_amount, Some(40));

    w2.wollet.add_details(&mut pset).unwrap();
    let mixed_signers = [
        &AnySigner::Software(signer.clone()),
        &AnySigner::Software(signer2),
    ];
    for s in mixed_signers {
        w.sign(s, &mut pset);
    }
    let tx = w.wollet.finalize(&mut pset).unwrap();
    let tx = serialize(&tx);
    assert!(env.elementsd_testmempoolaccept(&tx.to_hex()));
}

#[test]
fn test_multiple_reissuances() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);

    // Fund the wallet with 2 L-BTC UTXOs, one per issuance
    w.fund(&env, 100_000, None, None);
    w.fund(&env, 500_000, None, None);
    env.elementsd_generate(1);

    let utxos: Vec<_> = w
        .wollet
        .utxos()
        .unwrap()
        .iter()
        .map(|u| u.outpoint)
        .collect();
    assert_eq!(utxos.len(), 2);

    // Issue two assets, each with its reissuance token, in a single transaction, so that the
    // wallet owns both tokens. The first asset splits its 2 tokens across two outputs, so that
    // the wallet owns two utxos of the same token and a pin has something to choose between.
    let mut pset = w
        .tx_builder()
        .add_issuance(
            IssuanceRequest::new(10, 2)
                .add_token_output(1, None)
                .add_token_output(1, None),
        )
        .unwrap()
        .issue_asset(20, None, 2, None, None)
        .unwrap()
        .set_wallet_utxos(utxos)
        .finish()
        .unwrap();
    assert_eq!(pset.inputs().len(), 2);
    let (asset0, token0) = pset.inputs()[0].issuance_ids();
    let (asset1, token1) = pset.inputs()[1].issuance_ids();
    signer.sign(&mut pset).unwrap();
    w.send(&mut pset);
    assert_eq!(w.balance(&asset0), 10);
    assert_eq!(w.balance(&token0), 2);
    assert_eq!(w.balance(&asset1), 20);
    assert_eq!(w.balance(&token1), 2);
    env.elementsd_generate(1);

    // Reissuing the same asset twice in the same transaction is rejected
    let err = w
        .tx_builder()
        .add_reissuance(ReissuanceRequest::new(asset0, 5))
        .unwrap()
        .add_reissuance(ReissuanceRequest::new(asset0, 7))
        .unwrap_err();
    assert!(matches!(err, Error::DuplicatedReissuanceAsset(a) if a == asset0));

    // Reissuing zero units is rejected
    let err = w
        .tx_builder()
        .add_reissuance(ReissuanceRequest::new(asset0, 0))
        .unwrap_err();
    assert!(matches!(err, Error::InvalidAmount));

    // Two reissuances in the same transaction, the second one received by another wallet
    let signer2 = generate_signer();
    let view_key2 = generate_view_key();
    let desc2 = format!("ct({},elwpkh({}/*))", view_key2, signer2.xpub());
    let client2 = test_client_electrum(&env.electrum_url());
    let mut w2 = TestWollet::new(client2, &desc2);

    let mut pset = w
        .tx_builder()
        .add_reissuance(ReissuanceRequest::new(asset0, 5))
        .unwrap()
        .add_reissuance(ReissuanceRequest::new(asset1, 7).add_asset_output(7, Some(w2.address())))
        .unwrap()
        .finish()
        .unwrap();

    let details = w.wollet.get_details(&pset).unwrap();
    assert_eq!(n_issuances(&details), 0);
    assert_eq!(n_reissuances(&details), 2);

    // Each reissuance is assigned to the input holding the matching token, whose position depends
    // on coin selection, so compare them as a set rather than positionally
    let mut reissued: Vec<_> = details
        .issuances()
        .iter()
        .filter(|e| e.is_reissuance())
        .map(|e| {
            (
                e.asset().unwrap(),
                e.token().unwrap(),
                e.asset_satoshi().unwrap(),
            )
        })
        .collect();
    reissued.sort();
    let mut expected = vec![(asset0, token0, 5u64), (asset1, token1, 7u64)];
    expected.sort();
    assert_eq!(reissued, expected);

    // The first asset units are received by this wallet, the second ones are not, while both
    // tokens are spent and given back
    assert_eq!(*details.balances().get(&asset0).unwrap(), 5);
    assert!(!details.balances().contains_key(&asset1));
    assert!(!details.balances().contains_key(&token0));
    assert!(!details.balances().contains_key(&token1));

    signer.sign(&mut pset).unwrap();
    w.send(&mut pset);
    w2.sync();
    assert_eq!(w.balance(&asset0), 15);
    assert_eq!(w.balance(&asset1), 20);
    assert_eq!(w2.balance(&asset1), 7);
    assert_eq!(w.balance(&token0), 2);
    assert_eq!(w.balance(&token1), 2);

    // The wallet owns two utxos of the first reissuance token, so a pin picks which is spent
    let utxos = w.wollet.utxos().unwrap();
    let mut token0_utxos: Vec<_> = utxos
        .iter()
        .filter(|u| u.unblinded.asset == token0)
        .map(|u| u.outpoint)
        .collect();
    token0_utxos.sort();
    assert_eq!(token0_utxos.len(), 2);
    let lbtc_utxo = utxos
        .iter()
        .find(|u| u.unblinded.asset == w.policy_asset())
        .unwrap()
        .outpoint;

    // Outpoint of the single input carrying the reissuance
    let reissuance_input = |pset: &elements::pset::PartiallySignedTransaction| -> OutPoint {
        let inputs: Vec<_> = pset
            .inputs()
            .iter()
            .filter(|i| i.issuance_value_amount.is_some())
            .collect();
        assert_eq!(inputs.len(), 1);
        OutPoint::new(inputs[0].previous_txid, inputs[0].previous_output_index)
    };

    // With automatic coin selection, the pinned token utxo is the one added and reissued from
    for pinned in [token0_utxos[0], token0_utxos[1]] {
        let pset = w
            .tx_builder()
            .add_reissuance(ReissuanceRequest::new(asset0, 5).pin_input(pinned))
            .unwrap()
            .finish()
            .unwrap();
        assert_eq!(reissuance_input(&pset), pinned);
    }

    // Pinning an input that does not hold the reissuance token is rejected
    let err = w
        .tx_builder()
        .add_reissuance(ReissuanceRequest::new(asset0, 5).pin_input(lbtc_utxo))
        .unwrap()
        .finish()
        .unwrap_err();
    assert!(
        matches!(err, Error::ReissuancePinnedInputNotToken { outpoint, token } if outpoint == lbtc_utxo && token == token0)
    );

    // With a manual inputs order holding both token utxos, the pinned one is reissued from
    let selection = vec![token0_utxos[0], token0_utxos[1], lbtc_utxo];
    let pset = w
        .tx_builder()
        .add_reissuance(ReissuanceRequest::new(asset0, 5).pin_input(token0_utxos[1]))
        .unwrap()
        .set_wallet_utxos(selection.clone())
        .set_inputs_order(selection)
        .finish()
        .unwrap();
    assert_eq!(reissuance_input(&pset), token0_utxos[1]);

    // A manual inputs order omitting the pinned input is rejected
    let selection = vec![token0_utxos[0], lbtc_utxo];
    let err = w
        .tx_builder()
        .add_reissuance(ReissuanceRequest::new(asset0, 5).pin_input(token0_utxos[1]))
        .unwrap()
        .set_wallet_utxos(selection.clone())
        .set_inputs_order(selection)
        .finish()
        .unwrap_err();
    assert!(matches!(err, Error::ReissuanceOutpointNotInInputsOrder(o) if o == token0_utxos[1]));

    // The pinned reissuance is accepted by the chain, and the token is given back
    let mut pset = w
        .tx_builder()
        .add_reissuance(ReissuanceRequest::new(asset0, 5).pin_input(token0_utxos[1]))
        .unwrap()
        .finish()
        .unwrap();
    assert_eq!(reissuance_input(&pset), token0_utxos[1]);
    signer.sign(&mut pset).unwrap();
    w.send(&mut pset);
    env.elementsd_generate(1);
    assert_eq!(w.balance(&asset0), 20);
    assert_eq!(w.balance(&token0), 2);
}

#[test]
fn test_multiple_issuance_outputs() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({},elwpkh({}/*))", view_key, signer.xpub());
    let client = test_client_electrum(&env.electrum_url());
    let mut w = TestWollet::new(client, &desc);

    w.fund(&env, 100_000, None, None);
    env.elementsd_generate(1);

    // The outputs receiving the asset units must sum up to the issued amount
    let err = w
        .tx_builder()
        .add_issuance(IssuanceRequest::new(2, 1).add_asset_output(1, None))
        .unwrap_err();
    assert!(matches!(
        err,
        Error::IssuanceOutputsAmountMismatch {
            expected: 2,
            found: 1
        }
    ));

    // The same holds for the outputs receiving the reissuance tokens
    let err = w
        .tx_builder()
        .add_issuance(
            IssuanceRequest::new(2, 1)
                .add_token_output(1, None)
                .add_token_output(1, None),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        Error::IssuanceOutputsAmountMismatch {
            expected: 1,
            found: 2
        }
    ));

    // Outputs receiving nothing are rejected
    let err = w
        .tx_builder()
        .add_issuance(
            IssuanceRequest::new(2, 1)
                .add_asset_output(2, None)
                .add_asset_output(0, None),
        )
        .unwrap_err();
    assert!(matches!(err, Error::InvalidAmount));

    let asset_outputs = |tx: &WalletTx, asset: elements::AssetId| -> Vec<WalletTxOut> {
        tx.outputs
            .iter()
            .flatten()
            .filter(|o| o.unblinded.asset == asset)
            .cloned()
            .collect()
    };

    // Issue 2 asset units, split across two outputs of 1 unit each
    let mut pset = w
        .tx_builder()
        .add_issuance(
            IssuanceRequest::new(2, 1)
                .add_asset_output(1, None)
                .add_asset_output(1, None),
        )
        .unwrap()
        .finish()
        .unwrap();
    let (asset, token) = pset.inputs()[0].issuance_ids();
    assert_eq!(pset.inputs()[0].issuance_value_amount, Some(2));
    assert_eq!(pset.inputs()[0].issuance_inflation_keys, Some(1));

    signer.sign(&mut pset).unwrap();
    let txid = w.send(&mut pset);
    env.elementsd_generate(1);

    let tx = w.get_tx(&txid);
    let outputs = asset_outputs(&tx, asset);
    assert_eq!(outputs.len(), 2);
    assert!(outputs.iter().all(|o| o.unblinded.value == 1));
    // Outputs without an explicit address get a fresh address each
    assert_ne!(outputs[0].script_pubkey, outputs[1].script_pubkey);
    // The reissuance token is not split, so it lands on a single output
    assert_eq!(asset_outputs(&tx, token).len(), 1);
    assert_eq!(w.balance(&asset), 2);
    assert_eq!(w.balance(&token), 1);

    // Issue 2 reissuance tokens, split across two outputs of 1 token each, while the asset units
    // are received by an explicit address
    let asset_address = w.address();
    let mut pset = w
        .tx_builder()
        .add_issuance(
            IssuanceRequest::new(1, 2)
                .add_asset_output(1, Some(asset_address.clone()))
                .add_token_output(1, None)
                .add_token_output(1, None),
        )
        .unwrap()
        .finish()
        .unwrap();
    let (asset_2, token_2) = pset.inputs()[0].issuance_ids();
    assert_eq!(pset.inputs()[0].issuance_value_amount, Some(1));
    assert_eq!(pset.inputs()[0].issuance_inflation_keys, Some(2));

    signer.sign(&mut pset).unwrap();
    let txid = w.send(&mut pset);
    env.elementsd_generate(1);

    let tx = w.get_tx(&txid);
    let outputs = asset_outputs(&tx, token_2);
    assert_eq!(outputs.len(), 2);
    assert!(outputs.iter().all(|o| o.unblinded.value == 1));
    assert_ne!(outputs[0].script_pubkey, outputs[1].script_pubkey);
    let outputs = asset_outputs(&tx, asset_2);
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].script_pubkey, asset_address.script_pubkey());
    assert_eq!(w.balance(&asset_2), 1);
    assert_eq!(w.balance(&token_2), 2);

    // The outputs receiving the reissued units must sum up to the reissued amount too
    let err = w
        .tx_builder()
        .add_reissuance(ReissuanceRequest::new(asset, 2).add_asset_output(1, None))
        .unwrap_err();
    assert!(matches!(
        err,
        Error::IssuanceOutputsAmountMismatch {
            expected: 2,
            found: 1
        }
    ));

    // Reissue 2 units of the first asset, split across two outputs of 1 unit each
    let mut pset = w
        .tx_builder()
        .add_reissuance(
            ReissuanceRequest::new(asset, 2)
                .add_asset_output(1, None)
                .add_asset_output(1, None),
        )
        .unwrap()
        .finish()
        .unwrap();

    signer.sign(&mut pset).unwrap();
    let txid = w.send(&mut pset);

    let tx = w.get_tx(&txid);
    let outputs = asset_outputs(&tx, asset);
    assert_eq!(outputs.len(), 2);
    assert!(outputs.iter().all(|o| o.unblinded.value == 1));
    assert_ne!(outputs[0].script_pubkey, outputs[1].script_pubkey);
    assert_eq!(w.balance(&asset), 4);
    assert_eq!(w.balance(&token), 1);
}
