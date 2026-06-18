fn main() {
    // TODO: implement AMP2 mock server
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:0".to_string());

    eprintln!("amp2_mock listening on {addr}");
}
