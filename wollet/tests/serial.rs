use std::{str::FromStr, time::Duration};

use elements::bitcoin::bip32::DerivationPath;
use jade::{
    derivation_path_to_vec,
    mutex_jade::MutexJade,
    protocol::{GetXpubParams, JadeState},
    serialport, Jade,
};
use signer::Signer;

use crate::test_session::{setup, TestWollet};

#[test]
#[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
fn jade_send_lbtc() {
    let network = jade::Network::LocaltestLiquid;
    let ports = serialport::available_ports().unwrap();
    assert!(!ports.is_empty());
    let path = &ports[0].port_name;
    let port = serialport::new(path, 115_200)
        .timeout(Duration::from_secs(60))
        .open()
        .unwrap();

    let jade = Jade::new(port.into(), network);
    let mut jade = MutexJade::new(jade);

    let mut jade_state = jade.get_mut().unwrap().version_info().unwrap().jade_state;
    assert_ne!(jade_state, JadeState::Uninit);
    assert_ne!(jade_state, JadeState::Unsaved);
    if jade_state == JadeState::Locked {
        jade.unlock().unwrap();
        jade_state = jade.get_mut().unwrap().version_info().unwrap().jade_state;
    }
    assert_eq!(jade_state, JadeState::Ready);

    let server = setup();
    let path = "84h/1h/0h";
    let master_node = jade.get_mut().unwrap().get_master_xpub().unwrap();
    let fingerprint = master_node.fingerprint();
    let xpub = jade
        .get_xpub(GetXpubParams {
            network,
            path: derivation_path_to_vec(&DerivationPath::from_str(&format!("m/{path}")).unwrap()),
        })
        .unwrap();

    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";

    // m / purpose' / coin_type' / account' / change / address_index
    let desc_str = format!("ct(slip77({slip77_key}),elwpkh([{fingerprint}/{path}]{xpub}/1/*))");
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let signers = [&Signer::Jade(&jade)];

    let node_address = server.node_getnewaddress();
    wallet.send_btc(&signers, None, Some((node_address, 10_000)));

    // refuse the tx on the jade to keep the session logged
    jade.get_mut().unwrap().logout().unwrap();
}
