use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine};
use lwk_wollet::elements::hex::ToHex;

use crate::error::Error;

const COOKIE_USER: &str = "__cookie__";

/// Generates a fresh cookie file at `path`, replacing any previous one, and returns the
/// `Authorization` header value (`Basic <base64>`) clients must send to authenticate.
pub(crate) fn generate(path: &Path) -> Result<String, Error> {
    let secret: [u8; 32] = rand::random();
    let user_pass = format!("{COOKIE_USER}:{}", secret.to_hex());

    // TODO: restrict file permissions.
    // File permissions are a wider topic across lwk_cli/lwk_app (datadir, sqlite store,
    // state.json, etc), handle them properly with a shared util in a follow-up.
    std::fs::write(path, user_pass.as_bytes())?;

    Ok(header_value(&user_pass))
}

/// Reads the cookie file at `path`, if present, and returns the `Authorization` header value
/// to send.
pub(crate) fn read(path: &Path) -> Option<String> {
    let user_pass = std::fs::read_to_string(path).ok()?;
    Some(header_value(user_pass.trim()))
}

fn header_value(user_pass: &str) -> String {
    // b64 encoding improves interoperability eg with curl
    format!("Basic {}", STANDARD.encode(user_pass))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".cookie");

        // missing file
        assert!(read(&path).is_none());

        // roundtrip
        let generated = generate(&path).unwrap();
        let read_back = read(&path).unwrap();
        assert_eq!(generated, read_back);
        assert!(generated.starts_with("Basic "));

        // regenerating changes secret
        let regenerated = generate(&path).unwrap();
        assert_ne!(generated, regenerated);
    }
}
