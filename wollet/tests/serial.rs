use std::time::Duration;

use jade::{mutex_jade::MutexJade, protocol::GetXpubParams, serialport, Jade};
use signer::Signer;

use crate::test_session::{setup, TestWollet};

#[test]
#[ignore = "requires hardware jade: initialized with localtest network, locked and connected via usb/serial"]
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
    let jade = MutexJade::new(jade);

    jade.unlock().unwrap();

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
}
