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

use crate::{WolletDescriptor, EC};
use elements::bitcoin::bip32::{ChildNumber, DerivationPath, Fingerprint, KeySource, Xpub};
use elements::hashes::{sha256, Hash};
use elements::hex::ToHex;
use elements::pset::PartiallySignedTransaction;
use lwk_common::{keyorigin_xpub_from_str, Signer};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use url::Url;

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

/// The `purpose` field used to derive [`Amp2::elip153()`] keys: `1095585842`
/// (hex `0x414d5032`, bytes `b"AMP2"`).
const ELIP153_PURPOSE: u32 = 0x414d_5032;

/// Context for actions interacting with AMP2
#[derive(Debug)]
pub struct Amp2 {
    server_key: String,
    server_xpub: Xpub,
    server_path_from_master: DerivationPath,
    server_fingerprint: Fingerprint,
    url: String,
    is_mainnet: bool,
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

    /// Create an `Amp2Descriptor` using any `WolletDescriptor`
    ///
    /// Warning: AMP2 server only supports a limited subset of descriptors.
    /// To make sure this AMP2 descriptor can be used safely,
    /// register this with AMP2 as soon as possible.
    pub fn new_with_custom_descriptor(desc: WolletDescriptor) -> Self {
        Self::new(desc)
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
    /// Create a new AMP2 client
    ///
    ///  * `server_key` - The keyorigin xpub of the AMP2 server key
    ///  * `url` - The URL of the AMP2 server
    pub fn new(server_key: String, url: String) -> Result<Self, crate::Error> {
        Url::from_str(&url).map_err(crate::UrlError::Url)?;

        let (keysource, server_xpub) = keyorigin_xpub_from_str(&server_key)?;
        let (server_fingerprint, server_path_from_master) =
            keysource.ok_or(crate::Error::MissingKeyorigin)?;
        // TODO: per ELIP153 the server key should be the master xpub, allow it to have missing keyorigin
        // TODO: consider replacing server_key with server_keyorigin

        let is_mainnet = server_xpub.network.is_mainnet();
        Ok(Self {
            server_key,
            server_xpub,
            server_fingerprint,
            server_path_from_master,
            url,
            is_mainnet,
        })
    }

    /// Create a new AMP2 client with the default url and server key for the testnet network.
    pub fn new_testnet() -> Self {
        let server_xpub: Xpub = XPUB_TESTNET.parse().expect("valid xpub constant");
        let server_fingerprint: Fingerprint =
            FINGERPRINT_TESTNET.parse().expect("valid fingerprint");
        let server_path_from_master: DerivationPath =
            DERIVATION_PATH_TESTNET.parse().expect("valid path");
        Self {
            server_key: KEYORIGIN_XPUB_TESTNET.into(),
            server_xpub,
            server_fingerprint,
            server_path_from_master,
            url: URL_TESTNET.into(),
            is_mainnet: false,
        }
    }

    /// Create an AMP2 descriptor ELIP153 compliant from an LWK Signer.
    pub fn elip153_from_signer<S: Signer>(
        &self,
        signer: &S,
        account_num: u32,
    ) -> Result<Amp2Descriptor, crate::Error> {
        let user_path = self.elip153_user_path(account_num)?;
        let view_path = self.elip153_view_path(account_num)?;

        // TODO: map signer errors more nicely
        let fingerprint = signer
            .fingerprint()
            .map_err(|e| crate::Error::Generic(format!("{e:?}")))?;
        let user_xpub = signer
            .derive_xpub(&user_path)
            .map_err(|e| crate::Error::Generic(format!("{e:?}")))?;
        let view_xpub = signer
            .derive_xpub(&view_path)
            .map_err(|e| crate::Error::Generic(format!("{e:?}")))?;

        self.elip153(
            (fingerprint, user_path),
            user_xpub,
            (fingerprint, view_path),
            view_xpub,
        )
    }

    /// ELIP153 `USER_PATH = m/purpose'/coin_type'/account'`
    pub fn elip153_user_path(&self, account_num: u32) -> Result<DerivationPath, crate::Error> {
        let coin_type = if self.is_mainnet { 1776 } else { 1 };
        Ok(DerivationPath::from(vec![
            ChildNumber::from_hardened_idx(ELIP153_PURPOSE)?,
            ChildNumber::from_hardened_idx(coin_type)?,
            ChildNumber::from_hardened_idx(account_num)?,
        ]))
    }

    /// ELIP153 `SERVER_PATH`
    ///
    /// `user_xpub` must be the xpub derived at [`Amp2::elip153_user_path()`].
    fn elip153_server_path(&self, user_xpub: &Xpub) -> Result<DerivationPath, crate::Error> {
        let user_pubkey_hash =
            sha256::Hash::hash(&user_xpub.public_key.serialize()).to_byte_array();
        let server_path: Vec<ChildNumber> = user_pubkey_hash[..12]
            .chunks(4)
            .map(|chunk| {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(chunk);
                let masked = u32::from_be_bytes(bytes) & 0x7FFF_FFFF;
                // masked is below 2^31, so this cannot fail
                ChildNumber::from_normal_idx(masked)
            })
            .collect::<Result<_, _>>()?;
        Ok(DerivationPath::from(server_path))
    }

    /// ELIP153 `VIEW_PATH = m/purpose'/coin_type'/account'/server_fingerprint_masked'`
    pub fn elip153_view_path(&self, account_num: u32) -> Result<DerivationPath, crate::Error> {
        let user_path = self.elip153_user_path(account_num)?;
        let server_fingerprint_masked =
            u32::from_be_bytes(self.server_xpub.fingerprint().to_bytes()) & 0x7FFF_FFFF;
        Ok(user_path.child(ChildNumber::from_hardened_idx(server_fingerprint_masked)?))
    }

    /// Create an AMP2 descriptor ELIP153 compliant from a signer managed externally.
    ///
    /// Caller must ensure that:
    /// * `user_keyorigin_xpub` is the keyorigin xpub derived at [`Amp2::elip153_user_path()`] for `account_num`
    /// * `view_keyorigin_xpub` is the keyorigin xpub derived at [`Amp2::elip153_view_path()`] for `account_num`
    ///
    /// **Warning**: Passing incorrect signer data can lead to creating an incorrect
    /// descriptor, which could lead to loss of funds.
    pub fn elip153_from_external_signer(
        &self,
        account_num: u32,
        user_keyorigin_xpub: (KeySource, Xpub),
        view_keyorigin_xpub: (KeySource, Xpub),
    ) -> Result<Amp2Descriptor, crate::Error> {
        let ((user_fp, user_path), user_xpub) = user_keyorigin_xpub;
        let ((view_fp, view_path), view_xpub) = view_keyorigin_xpub;

        if user_fp != view_fp {
            return Err(crate::Error::Generic("fingerprint mismatch".to_string()));
        }
        if user_path != self.elip153_user_path(account_num)?
            || view_path != self.elip153_view_path(account_num)?
        {
            return Err(crate::Error::Generic(
                "unexpected keyorigin derivation path".to_string(),
            ));
        }

        self.elip153(
            (user_fp, user_path),
            user_xpub,
            (view_fp, view_path),
            view_xpub,
        )
    }

    fn elip153(
        &self,
        user_keysource: KeySource,
        user_xpub: Xpub,
        _view_keysource: KeySource,
        view_xpub: Xpub,
    ) -> Result<Amp2Descriptor, crate::Error> {
        // TODO: consider validating view_keysource

        let server_path = self.elip153_server_path(&user_xpub)?;
        let server_fingerprint = self.server_fingerprint;
        let server_derived_xpub = self.server_xpub.derive_pub(&EC, &server_path)?;
        let server_path_full = self.server_path_from_master.clone().extend(server_path);

        // Descriptor blinding key: hash the pubkey of the user key hardened-derived at
        // VIEW_PATH, so it's both deterministic and only computable by the user.
        let key_hash = sha256::Hash::hash(&view_xpub.public_key.serialize());
        let key_hex = key_hash.to_byte_array().to_hex();

        let user_xpub_str = format!("[{}/{}]{}", user_keysource.0, user_keysource.1, user_xpub);
        let server_xpub_str =
            format!("[{server_fingerprint}/{server_path_full}]{server_derived_xpub}");

        let s = format!(
            "ct({key_hex},elwsh(multi(2,{server_xpub_str}/<0;1>/*,{user_xpub_str}/<0;1>/*)))"
        );
        let descriptor: WolletDescriptor = s.parse()?;
        Ok(Amp2Descriptor::new(descriptor))
    }

    /// Get an AMP2 wallet descriptor from the keyorigin xpub string obtained from a signer
    pub fn descriptor_from_str(
        &self,
        keyorigin_xpub: &str,
        descriptor_blinding_key: &str,
    ) -> Result<Amp2Descriptor, crate::Error> {
        let (keysource, xpub) = keyorigin_xpub_from_str(keyorigin_xpub)?;
        let keysource = keysource.ok_or_else(|| crate::Error::MissingKeyorigin)?;
        self.descriptor(keysource, xpub, descriptor_blinding_key)
    }

    /// Get an AMP2 wallet descriptor
    pub fn descriptor(
        &self,
        user_keysource: KeySource,
        user_xpub: Xpub,
        descriptor_blinding_key: &str,
    ) -> Result<Amp2Descriptor, crate::Error> {
        // TODO; check Xpub network is consistent
        let amp2_xpub = &self.server_key;
        let user_xpub = format!("[{}/{}]{}", user_keysource.0, user_keysource.1, user_xpub);
        let s = format!(
            "ct({descriptor_blinding_key},elwsh(multi(2,{amp2_xpub}/<0;1>/*,{user_xpub}/<0;1>/*)))"
        );
        let descriptor: WolletDescriptor = s.parse()?;
        Ok(Amp2Descriptor::new(descriptor))
    }

    /// Register an AMP2 wallet with the AMP2 server
    pub async fn register(&self, desc: Amp2Descriptor) -> Result<RegisterResponse, crate::Error> {
        let body = RegisterRequest {
            descriptor: desc.descriptor().to_string(),
        };
        let url = format!("{}/wallets/register", self.url);
        let response = reqwest::Client::new().post(&url).json(&body).send().await?;
        if !response.status().is_success() {
            return Err(error_for_status(&url, response).await);
        }
        let j: RegisterResponse = response.json().await?;
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
        let url = format!("{}/wallets/register", self.url);
        let response = reqwest::blocking::Client::new()
            .post(&url)
            .json(&body)
            .send()?;
        if !response.status().is_success() {
            return Err(error_for_status_blocking(&url, response));
        }
        let j: RegisterResponse = response.json()?;
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
        let url = format!("{}/wallets/sign", self.url);
        let response = reqwest::Client::new().post(&url).json(&body).send().await?;
        if !response.status().is_success() {
            return Err(error_for_status(&url, response).await);
        }
        let j: CosignResponseInner = response.json().await?;
        let response: CosignResponse = j.try_into()?;
        let sigs_added =
            lwk_common::verify_added_sigs(pset, &response.pset, self.server_fingerprint, &EC)?;
        if sigs_added == 0 {
            return Err(crate::Error::Amp2NoSigsAdded);
        }
        Ok(response)
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
        let url = format!("{}/wallets/sign", self.url);
        let response = reqwest::blocking::Client::new()
            .post(&url)
            .json(&body)
            .send()?;
        if !response.status().is_success() {
            return Err(error_for_status_blocking(&url, response));
        }
        let j: CosignResponseInner = response.json()?;
        let response: CosignResponse = j.try_into()?;
        let sigs_added =
            lwk_common::verify_added_sigs(pset, &response.pset, self.server_fingerprint, &EC)?;
        if sigs_added == 0 {
            return Err(crate::Error::Amp2NoSigsAdded);
        }
        Ok(response)
    }
}

/// Builds an [`crate::Error`] describing an unsuccessful AMP2 HTTP response, including the
/// status code and (a bounded prefix of) the response body, so failures like a rejected
/// registration or cosign request are reported clearly rather than as a downstream JSON
/// parsing error.
async fn error_for_status(url: &str, response: reqwest::Response) -> crate::Error {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    let snippet: String = body.trim().chars().take(500).collect();
    crate::Error::Amp2HttpError {
        url: url.to_string(),
        status,
        body: (!snippet.is_empty()).then_some(snippet),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn error_for_status_blocking(url: &str, response: reqwest::blocking::Response) -> crate::Error {
    let status = response.status().as_u16();
    let body = response.text().unwrap_or_default();
    let snippet: String = body.trim().chars().take(500).collect();
    crate::Error::Amp2HttpError {
        url: url.to_string(),
        status,
        body: (!snippet.is_empty()).then_some(snippet),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Network;
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
        let descriptor_blinding_key =
            "slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67)";
        let expected = "ct(slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67),elwsh(multi(2,[3d970d04/87'/1'/0']tpubDC347GyKEGtyd4swZDaEmBTcNuqseyX7E3Yw58FoeV1njuBcUmBMr5vBeBh6eRsxKYHeCAEkKj8J2p2dBQQJwB8n33uyAPrdgwFxLFTCXRd/<0;1>/*,[c67f5991/87'/1'/0']tpubDC4SUtWGWcMQPtwjgQQ4DYnFmAYhiKxw3f3KKCvMGT9sojZNvHsQ4rVW6nQeCPtk4rLAxGKeuAzMmBmH92X3HDgLho3nRWpvuJrpCmYgeQj/<0;1>/*)))#6j2fne4s";

        let amp2 = Amp2::new_testnet();
        let desc = amp2
            .descriptor(keysource, xpub, descriptor_blinding_key)
            .unwrap();
        let desc1 = amp2
            .descriptor_from_str(keyorigin_xpub, descriptor_blinding_key)
            .unwrap();
        assert_eq!(desc.descriptor().to_string(), expected);
        assert_eq!(desc1.descriptor().to_string(), expected);
    }

    /// Derive an xpub at `path` and its keyorigin, standing in for what a hardware signer
    /// would return for the same request.
    fn derive(signer: &lwk_signer::SwSigner, path: &DerivationPath) -> (KeySource, Xpub) {
        use lwk_common::Signer;
        let xpub = signer.derive_xpub(path).unwrap();
        ((signer.fingerprint(), path.clone()), xpub)
    }

    #[test]
    fn amp2_desc_elip() {
        use lwk_signer::SwSigner;

        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer =
            SwSigner::new_with_network(mnemonic, lwk_common::Network::TestnetLiquid).unwrap();

        let amp2 = Amp2::new_testnet();
        let account = 0;
        let (user_keysource, user_xpub) =
            derive(&signer, &amp2.elip153_user_path(account).unwrap());
        let (view_keysource, view_xpub) =
            derive(&signer, &amp2.elip153_view_path(account).unwrap());

        let desc = amp2
            .elip153(user_keysource, user_xpub, view_keysource, view_xpub)
            .unwrap();

        let expected = "ct(4d90c104f07e6f4c3f2c2ef1100b2a24b93093eb3bdf975a85fbe2be5ddf7abe,elwsh(multi(2,[3d970d04/87'/1'/0'/2088330946/1132574986/2019598932]tpubDKX4imD1VZt8nMqqLWo2aBwJnJmw9kWhgob65LLKPd2UGcWZ2eCZXmVSM1uAzScUkFDVK3YdKZy49Qz7K1x2xEZ2AJhWaqnj25MbZSb4KYs/<0;1>/*,[73c5da0a/1095585842'/1'/0']tpubDDKAX9d8KBy2HJ5UTMg4xydwC7Jssy9qfnKrs5LTpM8PpBAiwqZ7k2GVA2P5kiWCPjnmHbDMxBng8FzDBHVqHpQkAwwc4VzXtGx1AY7zc9C/<0;1>/*)))#k8s8pxv0";
        assert_eq!(desc.descriptor().to_string(), expected);

        // elip153_from_signer must produce the exact same descriptor as the manual,
        // hardware-signer-compatible flow above.
        let desc_from_signer = amp2.elip153_from_signer(&signer, account).unwrap();
        assert_eq!(desc_from_signer.descriptor().to_string(), expected);
    }

    #[test]
    fn test_elip153_vectors() {
        // Generate ELIP153 test vectors with
        // cargo test -p lwk_wollet --features amp2 elip153_vectors -- --nocapture
        //
        // Note: expected_dwid must be taken from the ELIP153 test vectors to ensure
        // we keep following the spec
        use lwk_signer::SwSigner;

        let user_mnemonic_1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let user_mnemonic_2 =
            "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let server_mnemonic_1 =
            "letter advice cage absurd amount doctor acoustic avoid letter advice cage above";
        let server_mnemonic_2 = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";

        let mut i = 0;
        for (
            description,
            network,
            mnemonic,
            server_mnemonic,
            server_path_from_master,
            account,
            expected_dwid,
        ) in [
            (
                "Liquid",
                Network::Liquid,
                user_mnemonic_1,
                server_mnemonic_1,
                "",
                0u32,
                "4b2f-fca3-1d2e-0a8b-0d58-3ef5-9375-12cd",
            ),
            (
                "Testnet",
                Network::TestnetLiquid,
                user_mnemonic_1,
                server_mnemonic_1,
                "",
                0,
                "ded1-9ff5-4291-6309-3c91-a4d8-cfe5-8f74",
            ),
            (
                "Regtest",
                Network::default_regtest(),
                user_mnemonic_1,
                server_mnemonic_1,
                "",
                0,
                "a73a-c707-978f-cb7c-8912-5868-34c9-ee12",
            ),
            (
                "Liquid, different account",
                Network::Liquid,
                user_mnemonic_1,
                server_mnemonic_1,
                "",
                1,
                "5c5b-05fd-0fe1-58ef-b6a5-c54f-3e56-e478",
            ),
            (
                "Liquid, different user",
                Network::Liquid,
                user_mnemonic_2,
                server_mnemonic_1,
                "",
                0,
                "0328-ea41-7816-2f18-e86d-f110-bec4-f4eb",
            ),
            (
                "Liquid, different server",
                Network::Liquid,
                user_mnemonic_1,
                server_mnemonic_2,
                "",
                0,
                "7e5d-8182-ec4f-cf90-7be8-20f5-b3d8-8694",
            ),
            (
                "Liquid, non-master server xpub",
                Network::Liquid,
                user_mnemonic_1,
                server_mnemonic_1,
                "m/1h",
                0u32,
                "2d62-9c31-7d9a-1ba3-73ff-3c08-8803-aa1d",
            ),
        ] {
            i += 1;
            let signer = SwSigner::new_with_network(mnemonic, network).unwrap();
            let server_signer = SwSigner::new_with_network(server_mnemonic, network).unwrap();
            let server_path_from_master: DerivationPath = server_path_from_master.parse().unwrap();
            let server_xpub = server_signer.derive_xpub(&server_path_from_master).unwrap();
            let server_fp = server_signer.fingerprint();
            let server_keyorigin_xpub = if server_path_from_master.is_empty() {
                format!("[{server_fp}]{server_xpub}")
            } else {
                format!("[{server_fp}/{server_path_from_master}]{server_xpub}")
            };

            let amp2 = Amp2::new(server_keyorigin_xpub.clone(), URL_TESTNET.into()).unwrap();
            let (user_keysource, user_xpub) =
                derive(&signer, &amp2.elip153_user_path(account).unwrap());
            let (view_keysource, view_xpub) =
                derive(&signer, &amp2.elip153_view_path(account).unwrap());
            let desc = amp2
                .elip153(user_keysource, user_xpub, view_keysource, view_xpub)
                .unwrap();
            let dwid = desc.descriptor().dwid(network).unwrap();
            assert_eq!(dwid.to_string(), expected_dwid);
            let network_str = match network {
                Network::Liquid => "Liquid",
                Network::TestnetLiquid => "Liquid Testnet",
                Network::CustomElements(_) => "Liquid Regtest",
            };
            println!("* Test Vector {i}");
            println!("** Description: {description}");
            println!("** Network: {network_str}");
            println!("** User Mnemonic: <code>{mnemonic}</code>");
            println!("** AMP2 Server Xpub: <code>{server_keyorigin_xpub}</code>");
            println!("** User Account: {account}");
            println!("** CT Descriptor: <code>{}</code>", desc.descriptor());
            println!("** DWID: <code>{dwid}</code>\n");
        }
    }

    #[ignore]
    #[tokio::test]
    async fn amp2_network_calls() {
        let (keysource, xpub) = user_key();
        let descriptor_blinding_key =
            "slip77(0684e43749a3a3eb0362dcef8c66994bd51d33f8ce6b055126a800a626fc0d67)";
        let amp2 = Amp2::new_testnet();
        let d = amp2
            .descriptor(keysource, xpub, descriptor_blinding_key)
            .unwrap();
        let r = amp2.register(d).await.unwrap();
        assert!(!r.wid.is_empty());

        // TODO: test sign
    }

    #[test]
    fn amp2_new() {
        // Success case
        let amp2 = Amp2::new(KEYORIGIN_XPUB_TESTNET.to_string(), URL_TESTNET.to_string()).unwrap();

        assert_eq!(amp2.server_key, KEYORIGIN_XPUB_TESTNET);
        assert_eq!(amp2.url, URL_TESTNET);

        // Invalid URL
        let err =
            Amp2::new(KEYORIGIN_XPUB_TESTNET.to_string(), "fake_url".to_string()).unwrap_err();

        assert!(matches!(err, crate::Error::Url(_)));

        // Invalid keyorigin xpub
        let err = Amp2::new("fake_xpub".to_string(), URL_TESTNET.to_string()).unwrap_err();

        assert!(matches!(err, crate::Error::InvalidKeyOriginXpubError(_)));

        let desc_str = "ct(1111111111111111111111111111111111111111111111111111111111111111,elwsh(and_v(v:pk(026a04ab98d9e4774ad806e302dddeb63bea16b5cb5f223ee77478e861bb583eb3),multi(2,[342c8926/87h/1h/0h]tpubDDWUA7YvBHxdurKUrYFkdjsB59koHqvGRJ3j9zDhwMycxXHXz1ujTfHMB66K4rEWDM8BoDKDdJx3rVGp2qUSPnXVpQXi8qtnXqa96nPnZAH/0/*,[af9e5bc2/87h/1h/0h]tpubDDRPayLs2vBkRkyQ9X2BEhojxCy9vvZpjhubEVosz5pi66LuuAuyZQiUtsPBN5wSfhWLoMYM3gqVqT3Po4GpcWGUfPh8514ZBB9hfWFNEUA/0/*,[57411aec/87h/1h/0h]tpubDDmweWcTcRb54kZqy3Gv5JF8SjAyuoK3uPYXp24uz6nfsKjJojxjdZAang5HXDmtS8tg5CJntUC4fzn4aY5Dsg6Aphvq42vK9edmgX83NFg/0/*))))";
        let desc: WolletDescriptor = desc_str.parse().unwrap();
        let _amp2_desc = Amp2Descriptor::new_with_custom_descriptor(desc);
    }
}
