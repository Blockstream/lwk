use jade::{protocol::HandshakeParams, Jade};
use std::time::Duration;

#[test]
#[ignore = "requires hardware jade connected via usb/serial to input pin"]
fn auth_user() {
    let ports = serialport::available_ports().unwrap();
    if !ports.is_empty() {
        let path = &ports[0].port_name;
        let port = serialport::new(path, 115_200)
            .timeout(Duration::from_secs(60))
            .open()
            .unwrap();

        let mut jade_api = Jade::new(port.into(), jade::Network::Liquid);

        let result = jade_api.auth_user().unwrap();
        dbg!(&result);

        let url = result.urls()[0].as_str();
        dbg!(&url);
        let res = minreq::post(url).send().unwrap();
        let params: HandshakeParams = serde_json::from_slice(res.as_bytes()).unwrap();
        dbg!(&params);

        let result = jade_api.handshake_init(params).unwrap();
        dbg!(&result);
    }
}

#[cfg(feature = "serial")]
#[test]
#[ignore = "requires hardware jade connected via usb/serial that is already logged in"]
fn logout() {
    let ports = serialport::available_ports().unwrap();
    if !ports.is_empty() {
        let path = &ports[0].port_name;
        let port = serialport::new(path, 115_200)
            .timeout(Duration::from_secs(10))
            .open()
            .unwrap();

        let mut jade_api = Jade::new(port.into(), jade::Network::TestnetLiquid);

        let result = jade_api.logout().unwrap();
        dbg!(&result);
        assert!(result);
    }
}

#[test]
#[ignore = "requires hardware jade connected via usb/serial"]
fn ping() {
    let ports = serialport::available_ports().unwrap();
    if !ports.is_empty() {
        let path = &ports[0].port_name;
        let port = serialport::new(path, 115_200)
            .timeout(Duration::from_secs(10))
            .open()
            .unwrap();

        let mut jade_api = Jade::new(port.into(), jade::Network::TestnetLiquid);

        let result = jade_api.ping().unwrap();
        assert_eq!(result, 0);
    }
}
