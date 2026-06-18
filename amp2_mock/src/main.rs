use lwk_common::{Bip, Signer};
use lwk_signer::SwSigner;
use serde::Serialize;

#[derive(Serialize)]
struct InfoXpub {
    keyorigin_xpub: String,
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

    let server = tiny_http::Server::http(&addr).expect("failed to bind");
    let actual_addr = server.server_addr().to_ip().expect("not an IP address");
    eprintln!("amp2_mock listening on {actual_addr}");

    for request in server.incoming_requests() {
        let (status, body) = match request.url() {
            "/info/xpub" => {
                let resp = InfoXpub {
                    keyorigin_xpub: keyorigin_xpub.clone(),
                };
                (200, serde_json::to_string(&resp).unwrap())
            }
            _ => (404, r#"{"error":"not found"}"#.to_string()),
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
