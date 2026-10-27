//! Helpers for reading HTTP responses from untrusted servers.

use futures::StreamExt;
use reqwest::Response;

/// Read at most `limit` bytes of the body of `response`, and whether the body was longer.
///
/// The bytes are counted as the body streams in, `Content-Length` is not trusted, and the
/// download stops at the limit, so a server can't make us hold more than `limit` bytes.
pub(crate) async fn read_body_up_to(
    response: Response,
    limit: usize,
) -> Result<(Vec<u8>, bool), reqwest::Error> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let take = chunk.len().min(limit - body.len());
        body.extend_from_slice(&chunk[..take]);
        if take < chunk.len() {
            return Ok((body, true));
        }
    }
    Ok((body, false))
}
