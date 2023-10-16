use std::time::Duration;

use jade::{
    protocol::{HandshakeCompleteParams, HandshakeParams},
    serialport, Jade,
};

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

    let result = jade_api.auth_user().unwrap();
    dbg!(&result);

    let url = result.urls()[0].as_str();
    dbg!(&url);
    let resp = minreq::post(url).send().unwrap();
    assert_eq!(resp.status_code, 200);
    let params: HandshakeParams = serde_json::from_slice(resp.as_bytes()).unwrap();
    dbg!(&params);

    let result = jade_api.handshake_init(params).unwrap();
    dbg!(&result);

    let handshake_data = result.data();
    let data = serde_json::to_vec(&handshake_data).unwrap();
    let next_url = &result.urls()[0];
    let resp = minreq::post(next_url).with_body(data).send().unwrap();
    assert_eq!(resp.status_code, 200);
    let params: HandshakeCompleteParams = serde_json::from_slice(resp.as_bytes()).unwrap();

    dbg!(&params);

    let result = jade_api.handshake_complete(params).unwrap();
    assert!(result);

    //TODO start bitcoind/electrs, receive lbtc, send lbtc
}
