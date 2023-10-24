use bs_containers::testcontainers::clients::Cli;
use elements::bitcoin::bip32::DerivationPath;
use signer::Signer;
use std::str::FromStr;

use crate::{
    jade_emulator::inner_jade_debug_initialization,
    test_session::{generate_slip77, setup, TestWollet},
    TEST_MNEMONIC,
};

#[cfg(feature = "serial")]
mod serial {
    use jade::{mutex_jade::MutexJade, protocol::JadeState, serialport, Jade};
    use signer::Signer;
    use std::time::Duration;

    #[test]
    #[ignore = "requires hardware jade: initialized with localtest network, connected via usb/serial"]
    fn jade_burn() {
        let mut jade = init_and_unlock_serial_jade();
        let signers = [&Signer::Jade(&jade)];

        super::burn(&signers);

        jade.get_mut().unwrap().logout().unwrap();
    }

    fn init_and_unlock_serial_jade() -> MutexJade {
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
        jade
    }
}

#[test]
fn emul_burn() {
    let docker = Cli::default();
    let jade_init = inner_jade_debug_initialization(&docker, TEST_MNEMONIC.to_string());
    let signers = [&Signer::Jade(&jade_init.jade)];

    burn(&signers);
}

fn burn(signers: &[&Signer]) {
    let path = "84h/1h/0h";
    let master_node = signers[0].xpub().unwrap();
    let fingerprint = master_node.fingerprint();
    let xpub = signers[0]
        .derive_xpub(&DerivationPath::from_str(&format!("m/{path}")).unwrap())
        .unwrap();

    let slip77_key = generate_slip77();

    // m / purpose' / coin_type' / account' / change / address_index
    let desc_str = format!("ct(slip77({slip77_key}),elwpkh([{fingerprint}/{path}]{xpub}/1/*))");

    let server = setup();

    let mut wallet = TestWollet::new(&server.electrs.electrum_url, &desc_str);

    wallet.fund_btc(&server);
    let asset = wallet.fund_asset(&server);

    wallet.burnasset(signers, 1_000, &asset, None);
}
