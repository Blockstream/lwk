use crate::test_wollet::*;
use lwk_test_util::*;
use lwk_wollet::*;

#[test]
fn test_dumpwallet() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("dumpwallet.txt");
    let path_str = file_path.to_str().unwrap();
    env.elementsd_call("dumpwallet", &[path_str.into()]);

    let wd = WolletDescriptor::from_dumpwallet(path_str).unwrap();
    let client = test_client_electrum(&env.electrum_url());
    let mut wallet = TestWollet::new(client, &wd.to_string());
    wallet.sync();

    let explicit_utxos = wallet.wollet.explicit_utxos().unwrap();
    let explicit_balance: u64 = explicit_utxos.iter().map(|u| u.unblinded.value).sum();
    let conf_utxos = wallet.wollet.utxos().unwrap();
    let conf_balance: u64 = conf_utxos.iter().map(|u| u.unblinded.value).sum();
    assert!(explicit_balance + conf_balance > 0);
}
