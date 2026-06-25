use elements_miniscript::elements::bitcoin::bip32::{DerivationPath, Fingerprint, Xpub};
use std::str::FromStr;
use thiserror::Error;

/// The error type returned by keyorigin_xpub_from_str
#[derive(Error, Debug)]
#[error("Invalid key origin xpub \"{0}\", expected [fingerprint/path]xpub")]
pub struct InvalidKeyOriginXpub(String);

/// Parse a keyorigin xpub from a string
///
/// Example: "[73c5da0a/84h/1h/0h]tpub..."
pub fn keyorigin_xpub_from_str(
    s: &str,
) -> Result<(Option<(Fingerprint, DerivationPath)>, Xpub), InvalidKeyOriginXpub> {
    let (keyorigin, xpub) = match s.strip_prefix('[').and_then(|inner| inner.split_once(']')) {
        None => {
            let xpub = Xpub::from_str(s).map_err(|e| InvalidKeyOriginXpub(e.to_string()))?;
            return Ok((None, xpub));
        }
        Some((keyorigin, xpub)) => (keyorigin, xpub),
    };

    let (fingerprint, path) = keyorigin
        .split_once('/')
        .ok_or(InvalidKeyOriginXpub("unexpected format".to_string()))?;

    let fingerprint =
        Fingerprint::from_str(fingerprint).map_err(|e| InvalidKeyOriginXpub(e.to_string()))?;
    let path = DerivationPath::from_str(&format!("m/{}", path))
        .map_err(|e| InvalidKeyOriginXpub(e.to_string()))?;
    let xpub = Xpub::from_str(xpub).map_err(|e| InvalidKeyOriginXpub(e.to_string()))?;

    Ok((Some((fingerprint, path)), xpub))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_keyoriginxpub() {
        let fingerprint = "11a345ad";
        let path = "84h/1h/0h";
        let xpub = "tpubDCTUqRMrF2GHZ6pi5FcamdyGZ3oDJtQMH4y5Hyh8Uu7CQ3Zymbh1hpM84aXyJhgBhuh6WcUpKteMeYdyYfVUDRrsz8FUeRdoaaSRKkyMx6Y";

        let s = &format!("[{fingerprint}/{path}]{xpub}");
        keyorigin_xpub_from_str(s).unwrap();
        keyorigin_xpub_from_str(xpub).unwrap();

        for s in [
            &format!("{fingerprint}/{path}]{xpub}"),
            &format!("[[{fingerprint}/{path}]{xpub}"),
            &format!("x[{fingerprint}/{path}]{xpub}"),
            &format!("[{fingerprint}/{path}]]{xpub}"),
            &format!("[{fingerprint}]{xpub}"),
            &format!("[{fingerprint}-{path}]{xpub}"),
            &format!("[x1a345ad/{path}]{xpub}"),
            &format!("[{fingerprint}/x/{path}]{xpub}"),
            &format!("[{fingerprint}/{path}]1{xpub}"),
        ] {
            keyorigin_xpub_from_str(s).expect_err("test");
        }
    }
}
