//! Create and use AMP2 wallets.
//!
//! AMP2 is a service that allows issuers to create and manage authorized assets.
//!
//! If you want to receive an AMP2 asset, you need create and register an AMP2 wallet.
//! When you want to send an AMP2 asset, you need ask AMP2 to cosign the transaction, so that AMP2
//! can enforce the authorization rules.
//!
//! <div class="warning">
//! AMP2 is under development, expect breaking changes.
//! </div>

use crate::WolletDescriptor;
use elements::bitcoin::bip32::{KeySource, Xpub};

pub const FINGERPRINT_TESTNET: &str = "3d970d04";
pub const XPUB_TESTNET: &str = "tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd";
pub const DERIVATION_PATH_TESTNET: &str = "m/87h/1h/0h";
pub const KEYORIGIN_XPUB_TESTNET: &str = "[3d970d04/87h/1h/0h]tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd";
pub const URL_TESTNET: &str = "https://amp2.testnet.blockstream.com/";

/// AMP2 wallet
pub struct Amp2Wallet {
    descriptor: WolletDescriptor,
    #[allow(unused)]
    url: String,
}

impl Amp2Wallet {
    /// AMP2 wallet for Liquid Testnet
    pub fn new_testnet(user_keysource: KeySource, user_xpub: Xpub) -> Self {
        // TODO: allow to set custom blinding key
        let k = "slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67)";
        let amp2_xpub = KEYORIGIN_XPUB_TESTNET;
        let user_xpub = format!("[{}/{}]{}", user_keysource.0, user_keysource.1, user_xpub);
        let s = format!("ct({k},elwsh(multi(2,{amp2_xpub}/<0;1>/*,{user_xpub}/<0;1>/*)))");
        let descriptor: WolletDescriptor = s.parse().expect("fixed descriptor structure");
        Self {
            descriptor,
            url: URL_TESTNET.into(),
        }
    }

    pub fn descriptor(&self) -> WolletDescriptor {
        self.descriptor.clone()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use elements::bitcoin::bip32::{DerivationPath, Fingerprint};

    fn user_key() -> (KeySource, Xpub) {
        let fp: Fingerprint = "c67f5991".parse().unwrap();
        let path: DerivationPath = "m/87'/1'/0'".parse().unwrap();
        let keysource = (fp, path);
        let xpub: Xpub = "tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj".parse().unwrap();
        (keysource, xpub)
    }

    #[test]
    fn amp2_keyorigin() {
        let s = format!(
            "[{}/{}]{}",
            FINGERPRINT_TESTNET,
            &DERIVATION_PATH_TESTNET[2..],
            XPUB_TESTNET
        );
        assert_eq!(KEYORIGIN_XPUB_TESTNET, s);
    }

    #[test]
    fn amp2_desc() {
        let (keysource, xpub) = user_key();
        let expected = "ct(slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67),elwsh(multi(2,[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd/<0;1>/*,[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj/<0;1>/*)))#6j2fne4s";
        let amp2 = Amp2Wallet::new_testnet(keysource, xpub);
        assert_eq!(amp2.descriptor().to_string(), expected);
    }
}
