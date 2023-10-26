use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use client::Client;
use config::Config;
use signer::{Signer, SwSigner};
use tiny_jrpc::{tiny_http, JsonRpcServer, Request, Response};
use wollet::{ElementsNetwork, Wollet, EC};

pub mod client;
pub mod config;
pub mod consts;
pub mod error;
pub mod model;

#[derive(Default)]
pub struct State<'a> {
    // TODO: config is read-only, so it's not useful to wrap it in a mutex.
    // Ideally it should be in _another_ struct accessible by method_handler.
    pub config: Config,
    pub wollets: HashMap<String, Wollet>,
    pub signers: HashMap<String, Signer<'a>>,
}

pub struct App {
    rpc: JsonRpcServer,
    config: Config,
}

pub type Result<T> = std::result::Result<T, error::Error>;

impl App {
    pub fn new(config: Config) -> Result<App> {
        tracing::info!("Creating new app with config: {:?}", config);

        let state = Arc::new(Mutex::new(State {
            config: config.clone(),
            ..Default::default()
        }));
        let server = tiny_http::Server::http(config.addr)?;
        let rpc = tiny_jrpc::JsonRpcServer::new(server, state, method_handler);

        Ok(App { rpc, config })
    }

    pub fn addr(&self) -> SocketAddr {
        self.config.addr
    }

    pub fn join_threads(&mut self) {
        self.rpc.join_threads();
    }

    pub fn client(&self) -> Result<Client> {
        Client::new(self.config.addr)
    }
}

fn method_handler(request: Request, state: Arc<Mutex<State>>) -> tiny_jrpc::Result<Response> {
    let response = match request.method.as_str() {
        "generate_signer" => {
            let (_signer, mnemonic) = SwSigner::random(&EC)?;
            Response::result(
                request.id,
                serde_json::to_value(model::GenerateSignerResponse {
                    mnemonic: mnemonic.to_string(),
                })?,
            )
        }
        "version" => Response::result(
            request.id,
            serde_json::to_value(model::VersionResponse {
                version: consts::APP_VERSION.into(),
            })?,
        ),
        "load_wallet" => {
            let r: model::LoadWalletRequest =
                serde_json::from_value(request.params.unwrap_or_default())?;
            let mut s = state.lock().unwrap();
            let wollet = Wollet::new(
                ElementsNetwork::LiquidTestnet, // todo
                &s.config.electrum_url,         // electrum_url
                false,                          // tls
                false,                          // validate_domain
                &s.config.datadir,              // data root
                &r.descriptor,
            )?;
            let new = s.wollets.insert(r.descriptor.clone(), wollet).is_none();
            Response::result(
                request.id,
                serde_json::to_value(model::LoadWalletResponse {
                    descriptor: r.descriptor,
                    new,
                })?,
            )
        }
        "load_signer" => {
            let r: model::LoadSignerRequest =
                serde_json::from_value(request.params.unwrap_or_default())?;
            let signer = Signer::Software(SwSigner::new(&r.mnemonic, &EC)?);
            let fingerprint = signer.fingerprint()?.to_string();
            let xpub = signer.xpub()?;
            let mut s = state.lock().unwrap();
            // TODO: handle matching fingerprints
            let new = s.signers.insert(fingerprint.clone(), signer).is_none();
            Response::result(
                request.id,
                serde_json::to_value(model::LoadSignerResponse {
                    fingerprint,
                    new,
                    xpub,
                })?,
            )
        }
        _ => Response::unimplemented(request.id),
    };
    Ok(response)
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    fn app_random_port() -> App {
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let config = Config {
            addr,
            ..Default::default()
        };
        App::new(config).unwrap()
    }

    #[test]
    fn version() {
        let app = app_random_port();
        let addr = app.addr();
        let url = addr.to_string();
        dbg!(&url);

        let client = jsonrpc::Client::simple_http(&url, None, None).unwrap();
        let request = client.build_request("version", &[]);
        let response = client.send_request(request).unwrap();

        let result = response.result.unwrap().to_string();
        let actual: model::VersionResponse = serde_json::from_str(&result).unwrap();
        assert_eq!(actual.version, consts::APP_VERSION);
    }
}
