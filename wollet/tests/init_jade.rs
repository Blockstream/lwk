// TODO duplicated code existing also in jade crate under test

use bs_containers::{
    jade::{JadeEmulator, EMULATOR_PORT},
    testcontainers::{clients::Cli, Container},
};
use jade::{mutex_jade::MutexJade, protocol::DebugSetMnemonicParams, Jade};

#[derive(Debug)]
pub struct InitializedJade<'a> {
    _jade_emul: Container<'a, JadeEmulator>,
    pub jade: MutexJade,
}

pub fn inner_jade_debug_initialization(docker: &Cli, mnemonic: String) -> InitializedJade {
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into(), jade::Network::LocaltestLiquid);
    let params = DebugSetMnemonicParams {
        mnemonic,
        passphrase: None,
        temporary_wallet: false,
    };
    let result = jade_api.debug_set_mnemonic(params).unwrap();
    assert!(result);

    InitializedJade {
        _jade_emul: container,
        jade: MutexJade::new(jade_api),
    }
}

#[cfg(feature = "serial")]
pub mod serial {
    use jade::{mutex_jade::MutexJade, protocol::JadeState, serialport, Jade};
    use std::time::Duration;

    pub fn init_and_unlock_serial_jade() -> MutexJade {
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
