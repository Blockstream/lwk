use std::fmt;
use std::str::FromStr;

use elements_miniscript::elements::bitcoin::bip32::{DerivationPath, Fingerprint, Xpub};

use crate::keyorigin_xpub::{keyorigin_xpub_from_str, InvalidKeyOriginXpub};

/// A descriptor public key
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorPublicKey {
    keysource: Option<(Fingerprint, DerivationPath)>,
    xpub: Xpub,
}

impl FromStr for DescriptorPublicKey {
    type Err = InvalidKeyOriginXpub;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (keysource, xpub) = keyorigin_xpub_from_str(s)?;
        Ok(Self { keysource, xpub })
    }
}

impl DescriptorPublicKey {
    /// Master key fingerprint
    pub fn fingerprint(&self) -> Option<Fingerprint> {
        self.keysource.as_ref().map(|(fingerprint, _)| *fingerprint)
    }

    /// Derivation path from the master key
    pub fn derivation_path(&self) -> Option<&DerivationPath> {
        self.keysource.as_ref().map(|(_, path)| path)
    }

    /// Extended public key, without any key origin information
    pub fn xpub(&self) -> Option<Xpub> {
        // future extensions might also parse DescriptorPublicKey that are not xpubs
        Some(self.xpub)
    }
}

impl fmt::Display for DescriptorPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.keysource {
            Some((fingerprint, path)) => {
                if path.is_empty() {
                    write!(f, "[{fingerprint}]{}", self.xpub)
                } else {
                    write!(f, "[{fingerprint}/{path}]{}", self.xpub)
                }
            }
            None => write!(f, "{}", self.xpub),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const XPUB: &str = "tpubDCTUqRMrF2GHZ6pi5FcamdyGZ3oDJtQMH4y5Hyh8Uu7CQ3Zymbh1hpM84aXyJhgBhuh6WcUpKteMeYdyYfVUDRrsz8FUeRdoaaSRKkyMx6Y";
    const XPRV: &str = "tprv8bxtvyWEZW9M4n8ByZVSG2NNP4aeiRdhDZXNEv1eVNtrhLLnc6vJ1nf9DN5cHAoxMwqRR1CD6YXBvw2GncSojF8DknPnQVMgbpkjnKHkrGY";
    const PUBC: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const PUBX: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const WIFK: &str = "cTkFtf7sdcmunrJ983zPw68fr6sTrdjhYqPcb4gwgYw7SXrmnZam";

    #[test]
    fn test_dpk() {
        let xpub_bare = XPUB.to_string();
        let xpub_fp = format!("[11a345ad]{XPUB}");
        let xpub_fp_path = format!("[11a345ad/84'/1'/0']{XPUB}");

        let dpk = DescriptorPublicKey::from_str(&xpub_bare).unwrap();
        assert_eq!(dpk.to_string(), xpub_bare);
        assert!(dpk.xpub().is_some());
        assert!(dpk.fingerprint().is_none());
        assert!(dpk.derivation_path().is_none());

        let dpk = DescriptorPublicKey::from_str(&xpub_fp).unwrap();
        assert_eq!(dpk.to_string(), xpub_bare);
        assert!(dpk.xpub().is_some());
        assert!(dpk.fingerprint().is_some());
        assert!(dpk.derivation_path().is_none());

        let dpk = DescriptorPublicKey::from_str(&xpub_fp_path).unwrap();
        assert_eq!(dpk.to_string(), xpub_bare);
        assert!(dpk.xpub().is_some());
        assert!(dpk.fingerprint().is_some());
        assert!(dpk.derivation_path().is_some());

        // "h" are replaced by "'"
        let dpk = DescriptorPublicKey::from_str(&format!("[11a345ad/84h/1h/0h]{XPUB}")).unwrap();
        assert_eq!(dpk.to_string(), format!("[11a345ad/84'/1'/0']{XPUB}"));

        // unsupported
        let err = |s: &str| {
            assert!(DescriptorPublicKey::from_str(s).is_err());
        };
        err("not-a-key");
        err(&format!("{XPUB}/*"));
        err(&format!("{XPUB}/<0;1>"));
        err(&format!("{XPUB}/<0;1>/*"));
        err(&format!("[11a345ad/84'/1'/0']{XPUB}/<0;1>/*"));
        err(PUBC);
        err(PUBX);
        err(XPRV);
        err(WIFK);
    }
}
