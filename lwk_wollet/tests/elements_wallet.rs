use lwk_test_util::*;
use lwk_wollet::*;

#[test]
fn test_dumpwallet() {
    let env = TestEnvBuilder::from_env().with_electrum().build();

    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("dumpwallet.txt");
    let path_str = file_path.to_str().unwrap();
    env.elementsd_call("dumpwallet", &[path_str.into()]);
    let content = std::fs::read_to_string(path_str).unwrap();
    let mut master_xprv: bitcoin::bip32::Xpriv = content
        .lines()
        .find_map(|l| l.strip_prefix("# extended private masterkey: "))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    // Parse incorrectly sets the network
    master_xprv.network = bitcoin::NetworkKind::Test;
    let master_blinding_key: elements_miniscript::slip77::MasterBlindingKey = content
        .lines()
        .find_map(|l| l.strip_prefix("# Master private blinding key: "))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    // TODO: parse the other lines and check they're consistent
    let fingerprint = master_xprv.fingerprint(&EC);
    // Note: we can't use elements-miniscript and lwk with this descriptor
    let _desc = format!(
        "ct(slip77({master_blinding_key}),elwpkh([{fingerprint}/]{master_xprv}/0h/<0h;1h>/*h))"
    );

    let address = env.elementsd_getnewaddress();
    let info = env.elementsd_call("getaddressinfo", &[address.to_string().into()]);
    let path = info.get("hdkeypath").unwrap().as_str().unwrap();
    let path: bitcoin::bip32::DerivationPath = path.parse().unwrap();

    let xprv = master_xprv.derive_priv(&EC, &path).unwrap();
    let xpub = bitcoin::bip32::Xpub::from_priv(&EC, &xprv);
    // TODO: get script type from info
    // descriptor with no wildcards
    let desc =
        format!("ct(slip77({master_blinding_key}),elsh(wpkh([{fingerprint}/{path}]{xpub})))");
    let wd: WolletDescriptor = desc.parse().unwrap();
    let index = 0; // unused
    let params = &elements::AddressParams::ELEMENTS;
    let address_from_desc = wd.address(index, params).unwrap();
    assert_eq!(address, address_from_desc);
}
