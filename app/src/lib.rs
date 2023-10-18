use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use client::Client;
use config::Config;
use secp256k1::Secp256k1;
use tiny_jrpc::{tiny_http, JsonRpcServer, Request, Response};
use wollet::{ElementsNetwork, Wollet};

pub mod client;
pub mod config;
pub mod consts;
pub mod error;
pub mod model;

#[derive(Default)]
pub struct State {
    pub wollets: HashMap<String, Wollet>,
}

pub struct App {
    rpc: JsonRpcServer,
    config: Config,
}

pub type Result<T> = std::result::Result<T, error::Error>;

impl App {
    pub fn new(config: Config) -> Result<App> {
        tracing::info!("Creating new app with config: {:?}", config);

        let state = Arc::new(Mutex::new(State::default()));
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
    let secp = Secp256k1::default(); // todo: request context?
    let response = match request.method.as_str() {
        "generate_signer" => {
            let (_signer, mnemonic) = signer::SwSigner::random(&secp).unwrap(); // todo
            Response::result(
                request.id,
                serde_json::to_value(model::SignerGenerateResponse {
                    mnemonic: mnemonic.to_string(),
                })
                .unwrap(), // todo
            )
        }
        "version" => Response::result(
            request.id,
            serde_json::to_value(model::VersionResponse {
                version: consts::APP_VERSION.into(),
            })
            .unwrap(), // todo
        ),
        "load_wallet" => {
            let r: model::LoadWalletRequest =
                serde_json::from_value(request.params.unwrap()).unwrap();
            let wollet = Wollet::new(
                ElementsNetwork::LiquidTestnet, // todo
                "",                             // electrum url
                false,                          // tls
                false,                          // validate_domain
                "/tmp/.ks/",                    // data root
                &r.descriptor,
            )
            .unwrap();
            let mut s = state.lock().unwrap();
            let new = s.wollets.insert(r.descriptor.clone(), wollet).is_none();
            Response::result(
                request.id,
                serde_json::to_value(model::LoadWalletResponse {
                    descriptor: r.descriptor,
                    new,
                })
                .unwrap(), // todo
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
        let config = Config { addr };
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
