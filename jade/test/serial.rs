#[cfg(feature = "serial")]
mod test {
    use jade::{
        protocol::{HandshakeParams, Network},
        Jade,
    };
    use std::time::Duration;

    #[test]
    #[ignore = "requires hardware jade connected via usb/serial to input pin"]
    fn auth_user() {
        let ports = serialport::available_ports().unwrap();
        if !ports.is_empty() {
            let path = &ports[0].port_name;
            let port = serialport::new(path, 115_200)
                .timeout(Duration::from_secs(10))
                .open()
                .unwrap();

            let mut jade_api = Jade::new(port.into());

            let result = jade_api.auth_user(Network::Mainnet).unwrap();
            dbg!(&result);
            // insta::assert_yaml_snapshot!(result);

            let url = result.urls()[0].as_str();
            dbg!(&url);
            let res = ureq::post(url).call().unwrap();
            let params: HandshakeParams = res.into_json().unwrap();
            dbg!(&params);

            let result = jade_api.handshake_init(params).unwrap();
            dbg!(&result);
            // insta::assert_yaml_snapshot!(result);
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

            let mut jade_api = Jade::new(port.into());

            let result = jade_api.logout().unwrap();
            dbg!(&result);

            insta::assert_yaml_snapshot!(result);
        }
    }
}
