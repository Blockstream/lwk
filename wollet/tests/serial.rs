use std::time::Duration;

use jade::{
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
    let xpub = jade
        .get_xpub(GetXpubParams {
            network,
            path: vec![],
        })
        .unwrap();

    let slip77_key = "9c8e4f05c7711a98c838be228bcb84924d4570ca53f35fa1c793e58841d47023";
    let desc_str = format!("ct(slip77({}),elwpkh({}/*))", slip77_key, xpub);
    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);

    let signers = [&Signer::Jade(&jade)];

    wallet.send_btc(&signers, None);

    // refuse the tx on the jade to keep the session logged
    jade.get_mut().unwrap().logout().unwrap();
}
