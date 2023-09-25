use std::time::UNIX_EPOCH;

use jade::Jade;
use testcontainers::{clients, core::WaitFor, Image, ImageArgs};

const PORT: u16 = 30_121;

#[derive(Debug, Default)]
pub struct JadeEmulator;

#[derive(Clone, Debug, Default)]
pub struct Args;

impl ImageArgs for Args {
    fn into_iterator(self) -> Box<dyn Iterator<Item = String>> {
        let args = ["bash".to_string()];
        Box::new(args.into_iter())
    }
}

impl Image for JadeEmulator {
    type Args = ();

    fn name(&self) -> String {
        "xenoky/local-jade-emulator".into() // TODO Change with blockstream official jade emulator
    }

    fn tag(&self) -> String {
        "latest".into()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::StdOutMessage {
            message: "char device redirected".into(),
        }]
    }

    fn expose_ports(&self) -> Vec<u16> {
        [PORT].into()
    }
}

const _TEST_MNEMONIC: &str = "fish inner face ginger orchard permit
                             useful method fence kidney chuckle party
                             favorite sunset draw limb science crane
                             oval letter slot invite sadness banana";

#[test]
fn entropy() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator::default());
    let port = container.get_host_port_ipv4(PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let result = jade_api.add_entropy(&[1, 2, 3, 4]).unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn epoch() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator::default());
    let port = container.get_host_port_ipv4(PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let seconds = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let result = jade_api.set_epoch(seconds).unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn ping() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator::default());
    let port = container.get_host_port_ipv4(PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let result = jade_api.ping().unwrap();
    insta::assert_yaml_snapshot!(result);
}

#[test]
fn version() {
    let docker = clients::Cli::default();
    let container = docker.run(JadeEmulator::default());
    let port = container.get_host_port_ipv4(PORT);
    let stream = std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
    let mut jade_api = Jade::new(stream.into());

    let result = jade_api.version_info().unwrap();
    insta::assert_yaml_snapshot!(result);
}
