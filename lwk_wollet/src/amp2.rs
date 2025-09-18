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
use elements::pset::PartiallySignedTransaction;
use lwk_common::keyorigin_xpub_from_str;
use serde::{Deserialize, Serialize};

/// The fingerprint of the AMP2 server key for the testnet network.
pub const FINGERPRINT_TESTNET: &str = "3d970d04";
/// The xpub of the AMP2 server key for the testnet network.
pub const XPUB_TESTNET: &str = "tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd";
/// The derivation path of the AMP2 server key for the testnet network.
pub const DERIVATION_PATH_TESTNET: &str = "m/87h/1h/0h";
/// The keyorigin xpub of the AMP2 server key for the testnet network.
pub const KEYORIGIN_XPUB_TESTNET: &str = "[3d970d04/87h/1h/0h]tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd";
/// The URL of the AMP2 server for the testnet network.
pub const URL_TESTNET: &str = "https://amp2.testnet.blockstream.com/";

/// Context for actions interacting with AMP2
pub struct Amp2 {
    server_key: String,
    url: String,
}

/// An AMP2 descriptor
#[derive(Debug, Clone)]
pub struct Amp2Descriptor {
    inner: WolletDescriptor,
}

impl Amp2Descriptor {
    fn new(inner: WolletDescriptor) -> Self {
        Self { inner }
    }

    /// Return a copy of this Amp2 descriptor.
    pub fn descriptor(&self) -> WolletDescriptor {
        self.inner.clone()
    }
}

impl std::fmt::Display for Amp2Descriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[derive(Serialize)]
struct RegisterRequest {
    descriptor: String,
}

/// Response from the AMP2 server when registering a wallet
#[derive(Serialize, Deserialize)]
pub struct RegisterResponse {
    /// The AMP2 wallet id, should match [`WolletDescriptor::dwid()`]
    pub wid: String,
}

#[derive(Serialize)]
struct CosignRequest {
    pset: String,
}

#[derive(Deserialize)]
struct CosignResponseInner {
    pset: String,
}

/// Response from the AMP2 server when cosigning a PSET
#[derive(Serialize, Deserialize)]
pub struct CosignResponse {
    /// The cosigned PSET
    pub pset: PartiallySignedTransaction,
}

impl TryFrom<CosignResponseInner> for CosignResponse {
    type Error = crate::Error;

    fn try_from(r: CosignResponseInner) -> Result<CosignResponse, Self::Error> {
        let pset = r.pset.parse()?;
        Ok(CosignResponse { pset })
    }
}

impl Amp2 {
    /// Create a new AMP2 client with the default url and server key for the testnet network.
    pub fn new_testnet() -> Self {
        Self {
            server_key: KEYORIGIN_XPUB_TESTNET.into(),
            url: URL_TESTNET.into(),
        }
    }

    /// Get an AMP2 wallet descriptor from the keyorigin xpub string obtained from a signer
    pub fn descriptor_from_str(
        &self,
        keyorigin_xpub: &str,
    ) -> Result<Amp2Descriptor, crate::Error> {
        let (keysource, xpub) = keyorigin_xpub_from_str(keyorigin_xpub)?;
        let keysource = keysource.ok_or_else(|| crate::Error::MissingKeyorigin)?;
        Ok(self.descriptor(keysource, xpub))
    }

    /// Get an AMP2 wallet descriptor
    pub fn descriptor(&self, user_keysource: KeySource, user_xpub: Xpub) -> Amp2Descriptor {
        // TODO; check Xpub network is consistent
        // TODO: allow to set custom blinding key
        let k = "slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67)";
        let amp2_xpub = &self.server_key;
        let user_xpub = format!("[{}/{}]{}", user_keysource.0, user_keysource.1, user_xpub);
        let s = format!("ct({k},elwsh(multi(2,{amp2_xpub}/<0;1>/*,{user_xpub}/<0;1>/*)))");
        let descriptor: WolletDescriptor = s.parse().expect("fixed descriptor structure");
        Amp2Descriptor::new(descriptor)
    }

    /// Register an AMP2 wallet with the AMP2 server
    pub async fn register(&self, desc: Amp2Descriptor) -> Result<RegisterResponse, crate::Error> {
        let body = RegisterRequest {
            descriptor: desc.descriptor().to_string(),
        };
        let j: RegisterResponse = reqwest::Client::new()
            .post(format!("{}/wallets/register", self.url))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        Ok(j)
    }

    /// Register an AMP2 wallet with the AMP2 server
    #[cfg(not(target_arch = "wasm32"))]
    pub fn blocking_register(
        &self,
        desc: Amp2Descriptor,
    ) -> Result<RegisterResponse, crate::Error> {
        let body = RegisterRequest {
            descriptor: desc.descriptor().to_string(),
        };
        let j: RegisterResponse = reqwest::blocking::Client::new()
            .post(format!("{}/wallets/register", self.url))
            .json(&body)
            .send()?
            .json()?;
        Ok(j)
    }

    /// Ask the AMP2 server to cosign a PSET
    pub async fn cosign(
        &self,
        pset: &PartiallySignedTransaction,
    ) -> Result<CosignResponse, crate::Error> {
        let body = CosignRequest {
            pset: pset.to_string(),
        };
        let j: CosignResponseInner = reqwest::Client::new()
            .post(format!("{}/wallets/sign", self.url))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        j.try_into()
    }

    /// Ask the AMP2 server to cosign a PSET
    #[cfg(not(target_arch = "wasm32"))]
    pub fn blocking_cosign(
        &self,
        pset: &PartiallySignedTransaction,
    ) -> Result<CosignResponse, crate::Error> {
        let body = CosignRequest {
            pset: pset.to_string(),
        };
        let j: CosignResponseInner = reqwest::blocking::Client::new()
            .post(format!("{}/wallets/sign", self.url))
            .json(&body)
            .send()?
            .json()?;
        j.try_into()
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
        let keyorigin_xpub = "[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj";
        let expected = "ct(slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67),elwsh(multi(2,[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd/<0;1>/*,[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj/<0;1>/*)))#6j2fne4s";

        let amp2 = Amp2::new_testnet();
        let desc = amp2.descriptor(keysource, xpub);
        let desc1 = amp2.descriptor_from_str(keyorigin_xpub).unwrap();
        assert_eq!(desc.descriptor().to_string(), expected);
        assert_eq!(desc1.descriptor().to_string(), expected);
    }

    #[ignore]
    #[tokio::test]
    async fn amp2_network_calls() {
        let (keysource, xpub) = user_key();
        let amp2 = Amp2::new_testnet();
        let d = amp2.descriptor(keysource, xpub);
        let r = amp2.register(d).await.unwrap();
        assert!(!r.wid.is_empty());

        // TODO: test sign
    }
}
