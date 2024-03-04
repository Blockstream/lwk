use lwk_jade::{
    get_receive_address::{GetReceiveAddressParams, SingleOrMulti, Variant},
    protocol::JadeState,
    Jade, BAUD_RATE, TIMEOUT,
};
use std::str::FromStr;

fn serial_test_setup() -> Jade {
    lwk_test_util::init_logging();
    let ports = serialport::available_ports().unwrap();
    let path = &ports[0].port_name;
    let port = serialport::new(path, BAUD_RATE)
        .timeout(TIMEOUT)
        .open()
        .unwrap();

    Jade::new(port.into(), lwk_jade::Network::TestnetLiquid)
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial to input pin"]
fn unlock() {
    let jade_api = serial_test_setup();

    jade_api.unlock().unwrap();

    let result = jade_api.version_info().unwrap();
    assert_eq!(result.jade_state, JadeState::Ready);
    assert_eq!(result.jade_networks, "TEST".to_string());
    assert!(result.jade_has_pin);
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial that is already logged in"]
fn logout() {
    let jade_api = serial_test_setup();

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
    let jade_api = serial_test_setup();

    let result = jade_api.ping().unwrap();
    assert_eq!(result, 0);
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial"]
fn version_info() {
    let jade_api = serial_test_setup();

    let result = jade_api.version_info().unwrap();
    assert_eq!(result.jade_networks, "TEST".to_string());
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial"]
fn receive_address() {
    let jade_api = serial_test_setup();

    jade_api.unlock().unwrap();

    let params = GetReceiveAddressParams {
        network: lwk_jade::Network::TestnetLiquid,
        address: SingleOrMulti::Single {
            variant: Variant::ShWpkh,
            path: vec![2147483697, 2147483648, 2147483648, 0, 143],
        },
    };
    let result = jade_api.get_receive_address(params).unwrap();
    let address = elements::Address::from_str(&result).unwrap();
    assert!(address.blinding_pubkey.is_some());
    assert_eq!(address.params, &elements::AddressParams::LIQUID_TESTNET);
}
