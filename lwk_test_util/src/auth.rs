//! Authenticated Explorer API test stack: Keycloak (OAuth2 token issuer) + the Blockstream
//! APISIX gateway (`openid-connect` JWT enforcement) fronting a host upstream (e.g. the
//! Esplora HTTP API of the [`crate::TestEnv`] electrs).
//!
//! Both run as docker containers on a shared network so APISIX can reach Keycloak by container
//! name for OIDC discovery/JWKS; the test process reaches both via published localhost ports,
//! and the containers reach the host upstream via `host.docker.internal` (macOS) or the docker
//! network gateway (Linux), overridable with `LWK_TEST_UPSTREAM_HOST`.
//!
//! Containers are managed via the `docker` CLI (like [`crate::waterfalls::WaterfallsD`] manages
//! processes) rather than testcontainers so that [`AuthStack`] — and thus
//! [`crate::TestEnv`] — stays `Send + Sync`, which the bindings wrapper requires.

use std::io::Write;
use std::process::Command;
use std::time::Duration;

use rand::{thread_rng, Rng};
use tempfile::TempDir;

/// The realm imported in Keycloak, matching production.
pub const AUTH_REALM: &str = "blockstream-public";
/// The OAuth2 `client_credentials` client id defined in the test realm.
pub const AUTH_CLIENT_ID: &str = "lwk-test";
/// The client secret of [`AUTH_CLIENT_ID`].
pub const AUTH_CLIENT_SECRET: &str = "lwk-test-secret";
/// The fixed `user_uuid` claim emitted for tokens of [`AUTH_CLIENT_ID`] (credit accounting key).
pub const AUTH_USER_UUID: &str = "lwk-test-user-uuid";

const KEYCLOAK_PORT: u16 = 8_080;
const APISIX_PORT: u16 = 9_081;

/// Lean test realm modeled on the production one (`blockstream/keycloak-public`,
/// `setup/realms/blockstream-public.json`): same realm name, a confidential service-account
/// client with the `client_credentials` grant (like the dashboard-created API clients), the
/// audience mapper emitting `account`, and the `user_uuid` claim the credit-checker keys on.
/// Kept minimal on purpose — see the MR discussion; `KEYCLOAK_IMAGE_*` env vars allow swapping
/// in the production image if realm fidelity is ever needed.
const REALM_JSON: &str = include_str!("auth/realm.json");

fn keycloak_image() -> String {
    let name = std::env::var("KEYCLOAK_IMAGE_NAME")
        .unwrap_or_else(|_| "quay.io/keycloak/keycloak".to_string());
    // Same version as the base image of the production Keycloak (keycloak-public)
    let version = std::env::var("KEYCLOAK_IMAGE_VERSION").unwrap_or_else(|_| "26.5".to_string());
    format!("{name}:{version}")
}

fn apisix_image() -> String {
    let name =
        std::env::var("APISIX_IMAGE_NAME").unwrap_or_else(|_| "blockstream/apisix".to_string());
    let version =
        std::env::var("APISIX_IMAGE_VERSION").unwrap_or_else(|_| "2aa6756a-20251211".to_string());
    format!("{name}:{version}")
}

// Minimal APISIX static config: standalone (file) mode, only the plugins the test routes use.
const APISIX_CONFIG_YAML: &str = r#"
apisix:
  node_listen:
    - 9081
  enable_admin: false
  proxy_mode: http
nginx_config:
  error_log: "/dev/stderr"
  error_log_level: "warn"
deployment:
  role: data_plane
  role_data_plane:
    config_provider: yaml
plugins:
  - openid-connect
"#;

// Standalone routes: a catch-all requiring a valid JWT (validated against Keycloak's JWKS),
// proxying to the host upstream. Placeholders are substituted at runtime.
const APISIX_STANDALONE_YAML: &str = r#"
routes:
  - id: 1
    name: "esplora-auth"
    uri: "/*"
    plugins:
      openid-connect:
        client_id: "__CLIENT_ID__"
        client_secret: "dummy-secret-not-used-in-bearer-mode"
        realm: "__REALM__"
        discovery: "__DISCOVERY__"
        scope: "openid"
        bearer_only: true
        use_jwks: true
        set_access_token_header: false
        set_userinfo_header: false
        set_id_token_header: false
        set_refresh_token_header: false
    upstream:
      type: roundrobin
      scheme: http
      nodes:
        "__UPSTREAM__": 1
#END
"#;

/// Under gitlab CI mounting from the default tempdir may fail, use the project dir instead
/// (same workaround as `PinServer::tempdir`).
fn tempdir() -> Result<TempDir, std::io::Error> {
    match std::env::var("CI_PROJECT_DIR") {
        Ok(var) => TempDir::new_in(var),
        Err(_) => tempfile::tempdir(),
    }
}

/// Run `docker` with `args`, panicking (with stderr) on failure, returning stdout.
fn docker(args: &[&str]) -> String {
    let output = Command::new("docker")
        .args(args)
        .output()
        .expect("docker not available");
    assert!(
        output.status.success(),
        "docker {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A container started with `docker run -d`, removed on drop.
struct DockerContainer {
    name: String,
}

impl DockerContainer {
    fn run(args: &[&str]) -> DockerContainer {
        let name = args[args.iter().position(|a| *a == "--name").expect("--name") + 1];
        docker(&[&["run", "-d"], args].concat());
        DockerContainer {
            name: name.to_string(),
        }
    }

    /// The host port publishing `container_port`.
    fn host_port(&self, container_port: u16) -> u16 {
        let out = docker(&["port", &self.name, &container_port.to_string()]);
        // e.g. "0.0.0.0:55004\n[::]:55004"
        out.lines()
            .next()
            .and_then(|l| l.rsplit(':').next())
            .and_then(|p| p.parse().ok())
            .unwrap_or_else(|| panic!("cannot parse host port from '{out}'"))
    }
}

impl Drop for DockerContainer {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .output();
    }
}

/// Keycloak + APISIX fronting `upstream_port` on the host.
pub struct AuthStack {
    keycloak: DockerContainer,
    apisix: DockerContainer,
    network: String,
    keycloak_port: u16,
    apisix_port: u16,
    _keycloak_dir: TempDir,
    _apisix_dir: TempDir,
}

impl AuthStack {
    /// Start Keycloak and APISIX, with APISIX proxying authenticated requests to
    /// `upstream_port` on the host.
    ///
    /// Runs on a dedicated thread so it's callable from both sync and async (tokio) tests:
    /// the readiness polling uses `reqwest::blocking`, which panics inside an async context.
    pub fn new(upstream_port: u16) -> AuthStack {
        std::thread::spawn(move || Self::new_inner(upstream_port))
            .join()
            .expect("AuthStack setup thread panicked")
    }

    fn new_inner(upstream_port: u16) -> AuthStack {
        let suffix: u32 = thread_rng().gen();
        let network = format!("lwk-auth-{suffix}");
        let keycloak_name = format!("lwk-keycloak-{suffix}");
        let apisix_name = format!("lwk-apisix-{suffix}");

        docker(&["network", "create", &network]);

        // Keycloak: issues client_credentials tokens; APISIX validates against its JWKS.
        // KC_HOSTNAME pins the issuer to the in-network name: APISIX validates the token `iss`
        // claim against the discovery document, so it must not depend on which interface the
        // token was requested from (the host uses the published 127.0.0.1 port).
        let keycloak_dir = tempdir().expect("tempdir");
        let realm_path = keycloak_dir.path().join("realm.json");
        let mut file = std::fs::File::create(&realm_path).expect("create realm");
        file.write_all(REALM_JSON.as_bytes()).expect("write realm");
        let keycloak = DockerContainer::run(&[
            "--name",
            &keycloak_name,
            "--network",
            &network,
            "-e",
            "KC_BOOTSTRAP_ADMIN_USERNAME=admin",
            "-e",
            "KC_BOOTSTRAP_ADMIN_PASSWORD=admin",
            "-e",
            &format!("KC_HOSTNAME=http://{keycloak_name}:{KEYCLOAK_PORT}"),
            "-v",
            &format!(
                "{}:/opt/keycloak/data/import",
                keycloak_dir.path().display()
            ),
            "-p",
            &KEYCLOAK_PORT.to_string(),
            &keycloak_image(),
            "start-dev",
            "--import-realm",
        ]);
        let keycloak_port = keycloak.host_port(KEYCLOAK_PORT);

        let discovery_url = format!(
            "http://127.0.0.1:{keycloak_port}/realms/{AUTH_REALM}/.well-known/openid-configuration"
        );
        poll_until(&discovery_url, 200, 120, "keycloak realm");

        // APISIX: reaches Keycloak by container name on the shared network, and the upstream
        // on the host (see `upstream_host`).
        let upstream_host = upstream_host(&network);
        let standalone_yaml = APISIX_STANDALONE_YAML
            .replace("__CLIENT_ID__", AUTH_CLIENT_ID)
            .replace("__REALM__", AUTH_REALM)
            .replace(
                "__DISCOVERY__",
                &format!(
                    "http://{keycloak_name}:{KEYCLOAK_PORT}/realms/{AUTH_REALM}/.well-known/openid-configuration"
                ),
            )
            .replace("__UPSTREAM__", &format!("{upstream_host}:{upstream_port}"));
        let apisix_dir = tempdir().expect("tempdir");
        let config_path = apisix_dir.path().join("config.yaml");
        let mut file = std::fs::File::create(&config_path).expect("create config");
        file.write_all(APISIX_CONFIG_YAML.as_bytes())
            .expect("write config");
        let standalone_path = apisix_dir.path().join("apisix.yaml");
        let mut file = std::fs::File::create(&standalone_path).expect("create standalone");
        file.write_all(standalone_yaml.as_bytes())
            .expect("write standalone");
        let apisix = DockerContainer::run(&[
            "--name",
            &apisix_name,
            "--network",
            &network,
            "-v",
            &format!(
                "{}:/usr/local/apisix/conf/config.yaml",
                config_path.display()
            ),
            "-v",
            &format!(
                "{}:/usr/local/apisix/conf/apisix.yaml",
                standalone_path.display()
            ),
            "-p",
            &APISIX_PORT.to_string(),
            &apisix_image(),
        ]);
        let apisix_port = apisix.host_port(APISIX_PORT);

        // Without a token the gateway must answer 401: proves both that APISIX is up and that
        // the openid-connect plugin is active on the route.
        let gateway_probe = format!("http://127.0.0.1:{apisix_port}/blocks/tip/height");
        poll_until(&gateway_probe, 401, 60, "apisix gateway");

        AuthStack {
            keycloak,
            apisix,
            network,
            keycloak_port,
            apisix_port,
            _keycloak_dir: keycloak_dir,
            _apisix_dir: apisix_dir,
        }
    }

    /// The Keycloak OAuth2 token endpoint (reachable from the host).
    pub fn token_url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/realms/{AUTH_REALM}/protocol/openid-connect/token",
            self.keycloak_port
        )
    }

    /// Base url of the authenticated gateway fronting the upstream (reachable from the host).
    pub fn gateway_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.apisix_port)
    }

    /// Fetch a `client_credentials` access token from Keycloak, as an lwk client would.
    ///
    /// Runs on a dedicated thread so it's callable from both sync and async (tokio) tests.
    pub fn fetch_token(&self) -> String {
        let token_url = self.token_url();
        std::thread::spawn(move || Self::fetch_token_inner(token_url))
            .join()
            .expect("fetch_token thread panicked")
    }

    fn fetch_token_inner(token_url: String) -> String {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(token_url)
            .form(&[
                ("client_id", AUTH_CLIENT_ID),
                ("client_secret", AUTH_CLIENT_SECRET),
                ("grant_type", "client_credentials"),
                ("scope", "openid"),
            ])
            .send()
            .expect("token request");
        assert_eq!(response.status().as_u16(), 200, "token fetch failed");
        let json: serde_json::Value = response.json().expect("token json");
        json["access_token"]
            .as_str()
            .expect("access_token")
            .to_string()
    }

    /// The docker logs (stderr) of the APISIX container, for debugging.
    pub fn apisix_logs(&self) -> String {
        let output = Command::new("docker")
            .args(["logs", &self.apisix.name])
            .output()
            .expect("docker logs");
        String::from_utf8_lossy(&output.stderr).to_string()
    }
}

impl Drop for AuthStack {
    fn drop(&mut self) {
        // Containers must be removed before the network can be.
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.apisix.name, &self.keycloak.name])
            .output();
        let _ = Command::new("docker")
            .args(["network", "rm", &self.network])
            .output();
    }
}

/// Host through which containers reach the upstream listening in the test process.
///
/// - `LWK_TEST_UPSTREAM_HOST` wins if set
/// - macOS (Docker Desktop): `host.docker.internal`
/// - Linux (e.g. CI with a docker-in-docker service sharing the job's network namespace):
///   the docker network's gateway IP, which routes to the namespace where the test process
///   (and thus the upstream) listens on 0.0.0.0
fn upstream_host(network: &str) -> String {
    if let Ok(host) = std::env::var("LWK_TEST_UPSTREAM_HOST") {
        return host;
    }
    if cfg!(target_os = "macos") {
        return "host.docker.internal".to_string();
    }
    docker(&[
        "network",
        "inspect",
        network,
        "--format",
        "{{(index .IPAM.Config 0).Gateway}}",
    ])
}

/// Poll `url` until it returns `status`, panicking after `attempts` * 1s.
fn poll_until(url: &str, status: u16, attempts: u32, what: &str) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("client");
    for _ in 0..attempts {
        if let Ok(r) = client.get(url).send() {
            if r.status().as_u16() == status {
                return;
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    panic!("{what} not ready: '{url}' did not return {status} after {attempts}s");
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::{Read, Write};

    /// Self-contained smoke test of the auth stack (no elementsd/electrs needed): a valid
    /// Keycloak token is served through APISIX, missing/garbage tokens are rejected.
    #[test]
    #[ignore = "requires docker and the blockstream/apisix image"]
    fn auth_stack_smoke() {
        // Tiny host upstream standing in for esplora.
        let listener = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
        let upstream_port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok",
                );
            }
        });

        let stack = AuthStack::new(upstream_port);
        let url = format!("{}/blocks/tip/height", stack.gateway_url());
        let client = reqwest::blocking::Client::new();

        // missing token -> 401
        let r = client.get(&url).send().unwrap();
        assert_eq!(r.status().as_u16(), 401);

        // garbage token -> 401
        let r = client.get(&url).bearer_auth("not-a-jwt").send().unwrap();
        assert_eq!(r.status().as_u16(), 401);

        // valid Keycloak token -> served by the upstream
        let token = stack.fetch_token();
        let r = client.get(&url).bearer_auth(&token).send().unwrap();
        if r.status().as_u16() != 200 {
            println!(
                "www-authenticate: {:?}",
                r.headers().get("www-authenticate")
            );
            println!("apisix logs:\n{}", stack.apisix_logs());
        }
        assert_eq!(r.status().as_u16(), 200);
        assert_eq!(r.text().unwrap(), "ok");
    }
}
