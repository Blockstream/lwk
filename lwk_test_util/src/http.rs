use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};

/// Start a tiny local HTTP server for tests and return its base URL.
///
/// The server responds with the same status, content type, and body for every
/// request. If `keep_open` is true it keeps each accepted connection open after
/// writing the body, which is useful for testing stream cancellation.
pub fn serve_http_response(
    status_line: &'static str,
    content_type: &'static str,
    body: &'static str,
    keep_open: bool,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || loop {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 1024];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}"
        );
        let _ = stream.write_all(response.as_bytes());
        if keep_open {
            thread::sleep(Duration::from_secs(30));
        }
    });
    format!("http://{addr}")
}
