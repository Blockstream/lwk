use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use testcontainers::clients::Cli;
use testcontainers::images::postgres::Postgres;
use testcontainers::RunnableImage;

pub struct IndexerContext<'a> {
    _pg_container: testcontainers::Container<'a, Postgres>,
    _docker: &'a Cli,
    _tmpdir: tempfile::TempDir,
    _scanner: ChildGuard,
    _api: ChildGuard,
    api_url: String,
}

impl IndexerContext<'_> {
    pub fn api_url(&self) -> &str {
        &self.api_url
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn new(mut child: Child) -> Self {
        child.stdin.take();
        child.stdout.take();
        child.stderr.take();
        Self(Some(child))
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub async fn start_indexer<'a>(
    env: &lwk_test_util::TestEnv,
    cli: &'a Cli,
    binary: &Path,
    api_port: u16,
) -> IndexerContext<'a> {
    let policy_asset = env.elementsd_policy_asset();

    let host_path = std::env::current_dir()
        .unwrap()
        .join("tests")
        .join("migrations")
        .to_string_lossy()
        .to_string();

    let container_path = "/docker-entrypoint-initdb.d/";

    let pg_image = RunnableImage::from(Postgres::default())
        .with_volume((host_path, container_path.to_string()))
        .with_env_var(("POSTGRES_PASSWORD", "password"));

    let pg_container = cli.run(pg_image);
    let pg_port = pg_container.get_host_port_ipv4(5432);

    let esplora_url = env.esplora_url();
    let chain_height = env.elementsd_height();

    let tmpdir = tempfile::tempdir().unwrap();
    let config_dir = tmpdir.path().join("configuration");
    std::fs::create_dir(&config_dir).unwrap();

    let base_yaml = format!(
        r#"application:
  port: 8000
database:
  host: "127.0.0.1"
  port: {pg_port}
  username: "postgres"
  password: "password"
  database_name: "postgres"
esplora:
  base_url: "{esplora_url}"
  timeout: 10
  network: "regtest"
indexer:
  protocol_fee_keeper_asset_id: "{policy_asset}"
  interval: 200
  last_indexed_height: {chain_height}
"#
    );
    std::fs::write(config_dir.join("base.yaml"), &base_yaml).unwrap();
    std::fs::write(
        config_dir.join("local.yaml"),
        "application:\n  host: 127.0.0.1\n",
    )
    .unwrap();

    // Start scanner mode
    let scanner = ChildGuard::new(
        Command::new(binary)
            .current_dir(tmpdir.path())
            .env("RUN_MODE", "indexer")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start scanner"),
    );

    // Start API mode
    let api = ChildGuard::new(
        Command::new(binary)
            .current_dir(tmpdir.path())
            .env("RUN_MODE", "api")
            .env("APPLICATION__PORT", api_port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start API"),
    );

    // Wait for API to be ready
    let api_url = format!("http://127.0.0.1:{api_port}");
    let client = reqwest::Client::new();
    let mut ready = false;

    for _ in 0..10 {
        if let Ok(resp) = client.get(format!("{api_url}/health")).send().await {
            if resp.status().is_success() {
                ready = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(ready, "API health check failed to pass within timeout");

    IndexerContext {
        _docker: cli,
        _pg_container: pg_container,
        _tmpdir: tmpdir,
        _scanner: scanner,
        _api: api,
        api_url,
    }
}
