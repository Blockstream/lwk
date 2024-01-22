use jade::{protocol::JadeState, Jade, BAUD_RATE, TIMEOUT};

fn serial_test_setup() -> Jade {
    test_util::init_logging();
    let ports = serialport::available_ports().unwrap();
    let path = &ports[0].port_name;
    let port = serialport::new(path, BAUD_RATE)
        .timeout(TIMEOUT)
        .open()
        .unwrap();

    Jade::new(port.into(), jade::Network::TestnetLiquid)
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial to input pin"]
fn unlock() {
    let mut jade_api = serial_test_setup();

    jade_api.unlock().unwrap();

    let result = jade_api.version_info().unwrap();
    assert_eq!(result.jade_state, JadeState::Ready);
    assert_eq!(result.jade_networks, "TEST".to_string());
    assert!(result.jade_has_pin);
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial that is already logged in"]
fn logout() {
    let mut jade_api = serial_test_setup();

    let result = jade_api.logout().unwrap();
    assert!(result);

    let result = jade_api.version_info().unwrap();
    assert_eq!(result.jade_state, JadeState::Locked);
    assert_eq!(result.jade_networks, "TEST".to_string());
    assert!(result.jade_has_pin);
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial"]
fn ping() {
    let mut jade_api = serial_test_setup();

    let result = jade_api.ping().unwrap();
    assert_eq!(result, 0);
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial"]
fn version_info() {
    let mut jade_api = serial_test_setup();

    let result = jade_api.version_info().unwrap();
    assert_eq!(result.jade_networks, "TEST".to_string());
}
