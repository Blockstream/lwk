use lwk_containers::{Registry, REGISTRY_PORT};
use lwk_test_util::init_logging;
use testcontainers::clients;

#[test]
fn launch_test_registry() {
    init_logging();
    let docker = clients::Cli::default();
    let r = Registry::new(1111);
    let container: testcontainers::Container<'_, Registry> = docker.run(r);

    let port = container.get_host_port_ipv4(REGISTRY_PORT);
    assert!(port > 0);
}
