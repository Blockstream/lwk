use std::{borrow::Cow, collections::HashMap, env};

use testcontainers::{
    core::{error::Result, CmdWaitFor, ContainerState, ExecCommand, Host, Image, WaitFor},
    runners::SyncRunner,
    Container, ImageExt,
};
use testcontainers_modules::postgres::Postgres;
use url::Url;

/// The port exposed by the Liquid Asset Registry v2 container
const ASSET_REGISTRY_PORT: u16 = 8000;

fn container_url(url: &str) -> String {
    let mut url = Url::parse(url).expect("valid url");
    url.set_host(Some("host.docker.internal"))
        .expect("host.docker.internal is a valid host");
    url.to_string()
}

/// The Liquid Asset Registry v2 image
#[derive(Debug)]
pub struct AssetRegistryImage {
    name: String,
    tag: String,
    env_vars: HashMap<String, String>,
}

impl AssetRegistryImage {
    fn new(database_url: &str, esplora_url: &str) -> Self {
        let name = env::var("ASSET_REGISTRY_IMAGE_NAME")
            .unwrap_or("ghcr.io/blockstream/asset_registry_v2".into());
        let tag = env::var("ASSET_REGISTRY_IMAGE_VERSION").unwrap_or("latest".into());

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "ASSET_REGISTRY_DATABASE_URL".to_string(),
            database_url.into(),
        );
        env_vars.insert("ASSET_REGISTRY_ESPLORA_URL".to_string(), esplora_url.into());
        env_vars.insert(
            "ASSET_REGISTRY_ENFORCE_CHAIN_VERIFICATION".to_string(),
            "true".to_string(),
        );
        env_vars.insert(
            "ASSET_REGISTRY_ENFORCE_DOMAIN_VERIFICATION".to_string(),
            "false".to_string(),
        );
        env_vars.insert(
            "ASSET_REGISTRY_LEGACY_SHADOW_WRITE".to_string(),
            "false".to_string(),
        );
        Self {
            name,
            tag,
            env_vars,
        }
    }
}

impl Image for AssetRegistryImage {
    fn name(&self) -> &str {
        &self.name
    }

    fn tag(&self) -> &str {
        &self.tag
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        self.env_vars.iter()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stderr("Application startup complete")]
    }

    fn exec_after_start(&self, _cs: ContainerState) -> Result<Vec<ExecCommand>> {
        Ok(vec![ExecCommand::new(["alembic", "upgrade", "head"])
            .with_cmd_ready_condition(CmdWaitFor::exit_code(0))])
    }
}

/// A running instance of the Liquid Asset Registry v2 API
#[derive(Debug)]
pub struct AssetRegistry {
    // Drop order matters: `_registry` must be dropped before `_postgres`.
    _registry: Container<AssetRegistryImage>,
    _postgres: Container<Postgres>,
    url: String,
}

impl AssetRegistry {
    /// Starts the registry and its PostgreSQL database
    pub fn new(esplora_url: &str) -> Self {
        let suffix = format!("{:016x}", rand::random::<u64>());
        let network = format!("lwk-asset-registry-{suffix}");
        let postgres_name = format!("lwk-asset-registry-postgres-{suffix}");

        let postgres = Postgres::default()
            .with_host_auth()
            .with_network(network.clone())
            .with_container_name(postgres_name.clone())
            .start()
            .expect("failed to start postgres container");

        let esplora_url = container_url(esplora_url);
        let database_url = format!("postgresql+psycopg://postgres@{postgres_name}:5432/postgres");
        let registry = AssetRegistryImage::new(&database_url, &esplora_url)
            .with_network(network)
            .with_host("host.docker.internal", Host::HostGateway)
            .start()
            .expect("failed to start asset registry container");

        let port = registry
            .get_host_port_ipv4(ASSET_REGISTRY_PORT)
            .expect("asset registry port not exposed");
        let url = format!("http://127.0.0.1:{port}");

        Self {
            _registry: registry,
            _postgres: postgres,
            url,
        }
    }

    /// The base URL of the registry API
    pub fn url(&self) -> &str {
        &self.url
    }
}
