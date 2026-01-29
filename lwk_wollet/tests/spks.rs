use crate::test_wollet::*;
use elements::hex::ToHex;
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
    wallet.fund_btc(&env);

    let addr_from_wd = _wd
        .address(0, lwk_common::Network::LocaltestLiquid.address_params())
        .unwrap();
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
    // TODO: these do not match if there is no private blinding key

    let b = wallet.balance(&lbtc) as i64;
    assert!(b > 0);
    assert_eq!(*tx.balance.get(&lbtc).unwrap(), b);

    // TODO: fix Wollet.address(), which is currently broken for all cases
}
