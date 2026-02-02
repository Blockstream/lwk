use crate::test_wollet::*;
use elements::hex::ToHex;
use lwk_common::Signer;
use lwk_test_util::*;
use lwk_wollet::*;
use std::str::FromStr;

#[test]
fn test_spks() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", signer.xpub());
    let wd = WolletDescriptor::from_str(&desc).unwrap();
    let spk = wd.script_pubkey(Chain::External, 0).unwrap();
    let bk = lwk_common::derive_blinding_key(wd.ct_descriptor().unwrap(), &spk).unwrap();
    let desc = format!("{}:{}", bk.display_secret(), spk.to_hex());
    let _wd: WolletDescriptor = desc.parse().unwrap();

    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc);
    let lbtc = wallet.policy_asset();
    let sats = 1_000_000;
    let addr_from_wd = _wd
        .address(0, lwk_common::Network::LocaltestLiquid.address_params())
        .unwrap();
    wallet.fund(&env, sats, Some(addr_from_wd.clone()), None);

    let tx = &wallet.wollet.transactions().unwrap()[0];
    let addr_from_tx = tx
        .outputs
        .iter()
        .find(|o| o.is_some())
        .unwrap()
        .clone()
        .unwrap()
        .address;

    assert_eq!(addr_from_wd, addr_from_tx);

    let b = wallet.balance(&lbtc) as i64;
    assert!(b > 0);
    assert_eq!(*tx.balance.get(&lbtc).unwrap(), b);

    let addr = wallet.wollet.address(Some(0)).unwrap().address().clone();
    let change = wallet.wollet.change(Some(0)).unwrap().address().clone();
    assert_eq!(addr_from_wd, addr);
    assert_eq!(addr_from_wd, change);
    let err = wallet.wollet.address(Some(1)).unwrap_err();
    assert!(matches!(err, Error::IndexOutOfRange));
    let err = wallet.wollet.change(Some(1)).unwrap_err();
    assert!(matches!(err, Error::IndexOutOfRange));
    let err = wallet.wollet.address(None).unwrap_err();
    assert!(matches!(err, Error::UnsupportedWithoutDescriptor));
    let err = wallet.wollet.change(None).unwrap_err();
    assert!(matches!(err, Error::UnsupportedWithoutDescriptor));

    let node_address = env.elementsd_getnewaddress();

    let mut pset = wallet
        .wollet
        .tx_builder()
        .add_lbtc_recipient(&node_address, 10_000)
        .unwrap()
        .finish()
        .unwrap();

    // TODO: wollet: get_details: handle spks
    assert!(wallet.wollet.get_details(&pset).is_err());

    // Spks wollets do not automatically set the data necessary for the signer, which wont sign
    assert_eq!(signer.sign(&mut pset).unwrap(), 0);
}

#[test]
fn test_explicit_spks() {
    let env = TestEnvBuilder::from_env().with_electrum().build();
    let signer = generate_signer();
    let view_key = generate_view_key();
    let desc = format!("ct({view_key},elwpkh({}/*))", signer.xpub());
    let wd = WolletDescriptor::from_str(&desc).unwrap();
    let spk = wd.script_pubkey(Chain::External, 0).unwrap();
    let desc = format!(":{}", spk.to_hex());
    let wd = WolletDescriptor::from_str(&desc).unwrap();

    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &desc);
    let sats = 1_000_000;
    let addr_from_wd = wd
        .address(0, lwk_common::Network::LocaltestLiquid.address_params())
        .unwrap();
    wallet.fund_explicit(&env, sats, Some(addr_from_wd.clone()), None);

    let tx = &wallet.wollet.transactions().unwrap()[0];
    let addr_from_tx = tx
        .outputs
        .iter()
        .find(|o| o.is_some())
        .unwrap()
        .clone()
        .unwrap()
        .address;

    assert_eq!(addr_from_wd, addr_from_tx);
}
