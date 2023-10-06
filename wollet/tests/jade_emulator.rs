// TODO duplicated code existing also in jade crate under test

use bs_containers::{
    jade::{JadeEmulator, EMULATOR_PORT},
    testcontainers::{clients::Cli, Container},
};
use jade::{protocol::DebugSetMnemonicParams, Jade};

pub struct InitializedJade<'a> {
    _jade_emul: Container<'a, JadeEmulator>,
    pub jade: Jade,
}

pub fn inner_jade_debug_initialization(docker: &Cli, mnemonic: String) -> InitializedJade {
    let container = docker.run(JadeEmulator);
    let port = container.get_host_port_ipv4(EMULATOR_PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());
    let params = DebugSetMnemonicParams {
        mnemonic,
        passphrase: None,
        temporary_wallet: false,
    };
    let result = jade_api.debug_set_mnemonic(params).unwrap();
    assert!(result.get());

    InitializedJade {
        _jade_emul: container,
        jade: jade_api,
    }
}
