use std::time::Duration;

use jade::{serialport, Jade};

#[test]
#[ignore = "requires hardware jade connected via usb/serial "]
fn jade_send_lbtc() {
    let ports = serialport::available_ports().unwrap();
    assert!(!ports.is_empty());
    let path = &ports[0].port_name;
    let port = serialport::new(path, 115_200)
        .timeout(Duration::from_secs(60))
        .open()
        .unwrap();

    let mut jade_api = Jade::new(port.into(), jade::Network::LocaltestLiquid);

    let result = jade_api.unlock_jade().unwrap();

    assert!(result);

    //TODO start bitcoind/electrs, receive lbtc, send lbtc
}
