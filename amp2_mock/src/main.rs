use lwk_common::Network;
use lwk_common::{Bip, Signer};
use lwk_signer::SwSigner;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize)]
struct InfoXpub {
    keyorigin_xpub: String,
}

#[derive(Deserialize)]
struct RegisterRequest {
    descriptor: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    wid: String,
}

#[derive(Deserialize)]
struct SignRequest {
    pset: String,
}

#[derive(Serialize)]
struct SignResponse {
    pset: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:0".to_string());

    let mnemonic = std::env::var("AMP2_MNEMONIC").unwrap_or(
        "kind they sing appear whip boil divorce essence mask alien teach wire".to_string(),
    );

    let signer = SwSigner::new(&mnemonic, false).expect("invalid mnemonic");
    let keyorigin_xpub = signer
        .keyorigin_xpub(Bip::Bip87, false)
        .expect("failed to derive xpub");

    let server_xpub = signer
        .derive_xpub(
            &"m/87h/1h/0h"
                .parse()
                .expect("failed to parse derivation path"),
        )
        .expect("failed to derive xpub")
        .to_string();

    let server = tiny_http::Server::http(&addr).expect("failed to bind");
    let actual_addr = server.server_addr().to_ip().expect("not an IP address");
    eprintln!("amp2_mock listening on {actual_addr}");

    for mut request in server.incoming_requests() {
        let (status, body) = match request.url() {
            "/info/xpub" => {
                let resp = InfoXpub {
                    keyorigin_xpub: keyorigin_xpub.clone(),
                };
                (200, serde_json::to_string(&resp).unwrap())
            }
            "/wallets/register" => handle_register(&mut request, &server_xpub),
            "/wallets/sign" => handle_sign(&mut request, &signer),
            _ => (404, err_json("not found")),
        };

        let response = tiny_http::Response::from_string(body)
            .with_status_code(status)
            .with_header(
                "Content-Type: application/json"
                    .parse::<tiny_http::Header>()
                    .unwrap(),
            );
        let _ = request.respond(response);
    }
}

fn handle_register(request: &mut tiny_http::Request, server_xpub: &str) -> (u16, String) {
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        return (400, err_json("failed to read request body"));
    }

    let req: RegisterRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => return (400, err_json(&format!("invalid json: {e}"))),
    };

    let desc: lwk_wollet::WolletDescriptor = match req.descriptor.parse() {
        Ok(d) => d,
        Err(e) => return (400, err_json(&format!("invalid descriptor: {e}"))),
    };

    if !desc.to_string().contains(server_xpub) {
        return (400, err_json("descriptor does not contain the server key"));
    }

    let wid = match desc.dwid(Network::default_regtest()) {
        Ok(w) => w,
        Err(e) => return (400, err_json(&format!("invalid descriptor: {e}"))),
    };

    let resp = RegisterResponse { wid };
    (200, serde_json::to_string(&resp).unwrap())
}

fn handle_sign(request: &mut tiny_http::Request, signer: &SwSigner) -> (u16, String) {
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        return (400, err_json("failed to read request body"));
    }

    let req: SignRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => return (400, err_json(&format!("invalid json: {e}"))),
    };

    let mut pset = match PartiallySignedTransaction::from_str(&req.pset) {
        Ok(p) => p,
        Err(e) => return (400, err_json(&format!("invalid pset: {e}"))),
    };

    if let Err(e) = signer.sign(&mut pset) {
        return (400, err_json(&format!("signing failed: {e:?}")));
    }

    let resp = SignResponse {
        pset: pset.to_string(),
    };
    (200, serde_json::to_string(&resp).unwrap())
}

fn err_json(msg: &str) -> String {
    serde_json::to_string(&ErrorResponse {
        error: msg.to_string(),
    })
    .unwrap()
}
