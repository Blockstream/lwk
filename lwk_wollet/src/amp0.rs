//! Manage AMP0 wallets.

use aes::cipher::BlockEncryptMut;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
#[allow(deprecated)]
use aes_gcm::{aead::Aead, Aes256Gcm, Key, Nonce};
use base64::prelude::*;
use elements::bitcoin::bip32::{DerivationPath, Xpub};
use flate2::read::ZlibDecoder;
use hmac::Hmac;
use lwk_common::{Amp0SignerData, Network, Stream};
use pbkdf2::pbkdf2;
use rand::{thread_rng, RngCore};
use rmpv;
use scrypt::{scrypt, Params};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Sha512};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::str::FromStr;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::Mutex;

use crate::hashes::Hash;
use crate::wamp::common::{Arg, ClientRole, WampDict, WampId};
use crate::wamp::message::Msg;
use crate::EC;
use crate::{hex, Error};
use crate::{AddressResult, WolletDescriptor};
use elements::bitcoin::bip32::{ChainCode, ChildNumber, Fingerprint};
use elements::bitcoin::network::NetworkKind;
use elements::bitcoin::secp256k1::PublicKey;
use elements::encode::{deserialize, serialize};
use elements::hashes::sha256;
use elements::hex::{FromHex, ToHex};
use elements::pset::PartiallySignedTransaction;
use elements::secp256k1_zkp::{Generator, PedersenCommitment, SecretKey};
use elements::BlockHash;
use elements::Transaction;
use elements::{
    confidential::{AssetBlindingFactor, ValueBlindingFactor},
    Address, AssetId, TxOut, TxOutSecrets,
};
use elements::{script, Script, ScriptHash, WScriptHash};
use elements_miniscript::psbt::PsbtExt;

/// The fingerprint of the AMP0 server key for the mainnet network.
pub const AMP0_FINGERPRINT_MAINNET: &str = "0557d83a";
/// The fingerprint of the AMP0 server key for the testnet network.
pub const AMP0_FINGERPRINT_TESTNET: &str = "98c379b9";
/// The fingerprint of the AMP0 server key for the regtest network.
pub const AMP0_FINGERPRINT_REGTEST: &str = "b5281696";

struct Amp0Inner<S: Stream> {
    stream: S,
}

const BLOB_SALT: [u8; 8] = [0x62, 0x6c, 0x6f, 0x62, 0x73, 0x61, 0x6c, 0x74]; // 'blobsalt'
const WATCH_ONLY_SALT: [u8; 8] = [0x5f, 0x77, 0x6f, 0x5f, 0x73, 0x61, 0x6c, 0x74]; // '_wo_salt'
const WO_SEED_U: [u8; 8] = [0x01, 0x77, 0x6f, 0x5f, 0x75, 0x73, 0x65, 0x72]; // [1]'wo_user'
const WO_SEED_P: [u8; 8] = [0x02, 0x77, 0x6f, 0x5f, 0x70, 0x61, 0x73, 0x73]; // [2]'wo_pass'
const WO_SEED_K: [u8; 8] = [0x03, 0x77, 0x6f, 0x5f, 0x62, 0x6C, 0x6f, 0x62]; // [3]'wo_blob'

// TODO: add version or commit
const USER_AGENT: &str = "[v2,sw,csv,csv_opt] lwk";

// TODO: upload addesses at login and when creating an AMP0 account
// * internal: get _confidential_ address for Amp0Connected and Amp0LoggedIn
//   * tell green backend to monitor a new address: Amp0Inner.get_new_address
//   * get confidential address: derive the wollet descriptor
// * login: upload required CA addresses
// * create_amp0_account: upload INITIAL_UPLOAD_CA addresses
// Addresses uploaded after creation of 2of2_no_recovery subaccounts.
// const INITIAL_UPLOAD_CA: u32 = 20;

fn to_value<T: serde::Serialize>(value: &T) -> Result<rmpv::Value, Error> {
    let value = rmp_serde::encode::to_vec_named(value)?;
    Ok(rmp_serde::decode::from_slice(&value)?)
}

/// Green subaccount data returned at login
#[derive(Debug, Deserialize, Serialize)]
struct GreenSubaccount {
    /// Subaccount pointer
    pub pointer: u32,

    /// Subaccount type
    ///
    /// We're only interested in type "2of2_no_recovery"
    #[serde(rename = "type")]
    pub type_: String,

    /// Green Address ID, aka AMP ID
    #[serde(rename = "receiving_id")]
    pub gaid: String,

    /// Number of confidential addresses that should be uploaded for this subaccounts
    pub required_ca: u32,
}

/// Login Data returned by Green backend
///
/// Only the content that we use
#[derive(Debug, Deserialize, Serialize)]
struct LoginData {
    /// Derivation path used to derive the Green server xpub
    ///
    /// 128 hex chars
    pub gait_path: String,

    /// Key used to encrypt/decrypt the blob
    ///
    /// 128 hex chars
    /// Note: this key is itself encrypted
    pub wo_blob_key: Option<String>,

    /// Wallet subaccounts
    pub subaccounts: Vec<GreenSubaccount>,

    /// Blob hmac
    ///
    /// 32 bytes, base64 encoded
    pub client_blob_hmac: String,
}

/// Context for actions related to an AMP0 (sub)account
///
/// <div class="warning">
/// <b>WARNING:</b>
///
/// AMP0 is based on a legacy system, and some things do not fit precisely the way LWK allows to do
/// things.
///
/// Callers must be careful with the following:
/// * <b>Addresses: </b>
///   to get addresses use [`Amp0::address()`]. This ensures
///   that all addresses used are correctly monitored by the AMP0 server.
/// * <b>Syncing: </b>
///   to sync the AMP0 [`crate::Wollet`], use [`Amp0::last_index()`] and [`crate::clients::blocking::BlockchainBackend::full_scan_to_index()`]. This ensures that all utxos are synced, even if there are gaps between higher than the GAP LIMIT.
///
/// <i>
/// Failing to do the above might lead to inconsistent states, where funds are not shown or they
/// cannot be spent!
/// </i>
/// </div>
pub struct Amp0<S: Stream> {
    /// The LWK watch-only wallet descriptor corresponding to the AMP0 (sub)account.
    wollet_descriptor: WolletDescriptor,

    /// Green-backend actions
    amp0: Amp0Inner<S>,

    /// Network
    network: Network,

    /// AMP subaccount
    amp_subaccount: u32,

    /// AMP ID
    amp_id: String,

    /// Index of the last returned address
    last_index: u32,
}

#[cfg(not(target_arch = "wasm32"))]
impl Amp0<WebSocketClient> {
    /// Create a new AMP0 context
    pub async fn new_with_network(
        network: Network,
        username: &str,
        password: &str,
        amp_id: &str,
    ) -> Result<Self, Error> {
        let stream = stream_with_network(network).await?;
        Self::new(stream, network, username, password, amp_id).await
    }
}

impl<S: Stream> Amp0<S> {
    /// Create an AMP0 context
    ///
    /// `username` and `password` are the watch-only credentials as they're used in Blockstream
    /// App or with GDK.
    ///
    /// `amp_id` is a AMP0 subaccount GAID belonging to the wallet.
    /// If empty, the first AMP0 subaccount is used.
    pub async fn new(
        stream: S,
        network: Network,
        username: &str,
        password: &str,
        amp_id: &str,
    ) -> Result<Self, Error> {
        // connect to ga-backend
        let amp0 = Amp0Inner::new(stream).await?;
        // login.watch_only_v2
        // parse login data
        let login_data = amp0.login(username, password).await?;

        // get amp account
        let subaccount = login_data
            .subaccounts
            .iter()
            .find(|s| s.type_ == "2of2_no_recovery" && (amp_id.is_empty() || s.gaid == amp_id))
            .ok_or_else(|| Error::Generic("Missing AMP subaccount".into()))?;
        let amp_subaccount = subaccount.pointer;
        let amp_id = subaccount.gaid.clone();

        // get blob
        let blob64 = amp0.get_blob().await?;
        // decrypt blob
        let wo_blob_key_hex = login_data
            .wo_blob_key
            .ok_or_else(|| Error::Generic("Missing wo_blob_key".into()))?;
        let enc_key = decrypt_blob_key(username, password, &wo_blob_key_hex)?;
        let blob = Blob::from_base64(&blob64, &enc_key)?;
        // compute wallet descriptor
        let gait_path = &login_data.gait_path;
        let desc = amp_descriptor(&blob, amp_subaccount, &network, gait_path)?;

        let wollet_descriptor = WolletDescriptor::from_str(&desc)?;

        // get last index
        let (last_index, _script) = amp0.get_new_address(amp_subaccount).await?;

        let mut amp0 = Self {
            wollet_descriptor,
            amp0,
            network,
            amp_subaccount,
            amp_id,
            last_index,
        };

        amp0.upload_ca(subaccount.required_ca).await?;

        Ok(amp0)
    }

    async fn upload_ca(&mut self, required_ca: u32) -> Result<(), Error> {
        if required_ca > 0 {
            let mut addresses = vec![];
            for _ in 0..required_ca {
                let addr = self.address(None).await?;
                addresses.push(addr.address().to_string());
            }
            self.amp0.upload_ca(self.amp_subaccount, &addresses).await?;
        }
        Ok(())
    }

    /// Index of the last returned address.
    ///
    /// Use this and [`crate::clients::blocking::BlockchainBackend::full_scan_to_index()`] to sync the `Wollet`
    pub fn last_index(&self) -> u32 {
        self.last_index
    }

    /// Account AMP ID
    pub fn amp_id(&self) -> &str {
        &self.amp_id
    }

    /// The LWK watch-only wallet descriptor corresponding to the AMP0 (sub)account.
    ///
    /// <div class="warning">
    /// <b>WARNING:</b>
    ///
    /// Do not derive addresses using [`WolletDescriptor::address()`] or [`crate::Wollet::address()`].
    ///
    /// See [`Amp0`] for more details.
    /// </div>
    pub fn wollet_descriptor(&self) -> WolletDescriptor {
        self.wollet_descriptor.clone()
    }

    /// Get an address
    ///
    /// If `index` is None, a new address is returned.
    pub async fn address(&mut self, index: Option<u32>) -> Result<AddressResult, Error> {
        match index {
            Some(i) => {
                if i == 0 {
                    return Err(Error::Generic("Invalid address index for AMP0".into()));
                }
                if i > self.last_index {
                    return Err(Error::Generic("Address index too high".into()));
                }
                let address = self
                    .wollet_descriptor
                    .amp0_address(i, self.network.address_params())?;
                Ok(AddressResult::new(address, i))
            }
            None => {
                // Get a new address from Green server
                let (pointer, script) = self.amp0.get_new_address(self.amp_subaccount).await?;
                let wsh = script::Builder::new()
                    .push_int(0)
                    .push_slice(&WScriptHash::hash(script.as_bytes())[..])
                    .into_script();
                let sh = ScriptHash::hash(wsh.as_bytes());
                let spk = Script::new_p2sh(&sh);

                // Get address from the LWK wollet
                let address = self
                    .wollet_descriptor
                    .amp0_address(pointer, self.network.address_params())?;

                if address.script_pubkey() != spk {
                    return Err(Error::Generic("Unexpected address".into()));
                }

                // Update last index
                self.last_index = pointer;
                Ok(AddressResult::new(address, pointer))
            }
        }
    }

    // Green backend http URL
    fn http_url(&self) -> &'static str {
        match self.network {
            Network::Liquid => "https://green-liquid-mainnet.blockstream.com",
            Network::TestnetLiquid => "https://green-liquid-testnet.blockstream.com",
            Network::LocaltestLiquid => "http://127.0.0.1:9908",
        }
    }

    /// Ask AMP0 server to cosign
    pub async fn sign(&self, amp0pset: &Amp0Pset) -> Result<Transaction, Error> {
        let blinding_nonces = amp0pset.blinding_nonces().to_vec();

        // "finalize" the PSET for Green/AMP0
        let mut pset = amp0pset.pset().clone();
        let mut scripts = vec![];

        // Dummy signature to use a placeholder
        let dummy_hex = "304402207f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f02207f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f01";
        let dummy = Vec::<u8>::from_hex(dummy_hex)?;

        for input in pset.inputs_mut() {
            // FIXME: ignore/handle non Green/AMP0 inputs
            for pk in input.bip32_derivation.keys() {
                if !input.partial_sigs.contains_key(pk) {
                    input.partial_sigs.insert(*pk, dummy.clone());
                }
            }

            // Extract the witness scripts (required by the cosigning API)
            let script = input
                .witness_script
                .as_ref()
                .map(|s| s.to_hex())
                .unwrap_or_default();
            scripts.push(script);
        }

        let _ = pset.finalize_mut(&EC, BlockHash::all_zeros());

        let tx = pset.extract_tx()?;
        let tx_hex = serialize(&tx).to_hex();

        #[derive(serde::Serialize)]
        struct DelayedSignatureRequest {
            tx: String,
            blinding_nonces: Vec<String>,
            scripts: Vec<String>,
        }

        #[derive(serde::Deserialize)]
        struct DelayedSignatureResponse {
            result: bool,
            error: String,
            tx: Option<String>,
        }

        let body = DelayedSignatureRequest {
            tx: tx_hex,
            blinding_nonces,
            scripts,
        };

        let j: DelayedSignatureResponse = reqwest::Client::new()
            .post(format!("{}/delayed_signature", self.http_url()))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        if !j.result {
            return Err(Error::Generic(format!(
                "delayed_signature: error: {}",
                j.error
            )));
        }

        let tx = j.tx.unwrap_or_default();
        let tx: Transaction = deserialize(&Vec::<u8>::from_hex(&tx)?)?;
        Ok(tx)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Amp0Connected<WebSocketClient> {
    /// Connect and register to AMP0
    pub async fn new_(network: Network, signer_data: Amp0SignerData) -> Result<Self, Error> {
        let stream = stream_with_network(network).await?;
        Self::new(stream, network, signer_data).await
    }
}

/// Session connecting to AMP0
pub struct Amp0Connected<S: Stream> {
    amp0: Amp0Inner<S>,
    network: Network,
    signer_data: Amp0SignerData,
}

impl<S: Stream> Amp0Connected<S> {
    /// Connect and register to AMP0
    pub async fn new(
        stream: S,
        network: Network,
        signer_data: Amp0SignerData,
    ) -> Result<Self, Error> {
        let amp0 = Amp0Inner::new(stream).await?;
        let master_xpub = signer_data.master_xpub();
        let gait_path = derive_gait_path(signer_data.register_xpub());
        amp0.register(master_xpub, &gait_path).await?;
        Ok(Self {
            amp0,
            network,
            signer_data,
        })
    }

    /// Obtain a login challenge
    ///
    /// This must be signed with [`lwk_common::Amp0Signer::amp0_sign_challenge()`].
    pub async fn get_challenge(&self) -> Result<String, Error> {
        let login_address = self.signer_data.login_address(&self.network);
        self.amp0.get_challenge(&login_address).await
    }

    /// Log in
    ///
    /// `sig` must be obtained from [`lwk_common::Amp0Signer::amp0_sign_challenge()`] called with the value returned
    /// by [`Amp0Connected::get_challenge()`]
    pub async fn login(self, sig: &str) -> Result<Amp0LoggedIn<S>, Error> {
        let login_data = self.amp0.authenticate(sig).await?;
        // TODO: check that login data is consistent with what we passed
        let (enc_key, hmac_key) = derive_blob_keys(self.signer_data.client_secret_xpub());
        let zero_hmac_b64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        let (blob, hmac) = if login_data.client_blob_hmac == zero_hmac_b64 {
            // No blob, create and set a new one
            let blob = Blob::new(&self.signer_data)?;
            let blob64 = blob.to_base64(&enc_key)?;
            let hmac = compute_hmac(&hmac_key, &blob64)?;
            self.amp0.set_blob(&blob64, &hmac, zero_hmac_b64).await?;
            (blob, hmac)
        } else {
            // Get the blob
            let blob64 = self.amp0.get_blob().await?;
            let blob = Blob::from_base64(&blob64, &enc_key)?;
            let hmac = compute_hmac(&hmac_key, &blob64)?; // can be extracted from login data
            (blob, hmac)
        };
        // TODO: upload ca if needed
        Ok(Amp0LoggedIn {
            amp0: self.amp0,
            login_data,
            blob,
            hmac,
            enc_key: enc_key.to_vec(),
            hmac_key: hmac_key.to_vec(),
        })
    }
}

/// Session logged in to AMP0
pub struct Amp0LoggedIn<S: Stream> {
    amp0: Amp0Inner<S>,
    //  TODO: consider parsing this data further
    login_data: LoginData,
    blob: Blob,
    hmac: String,
    enc_key: Vec<u8>,
    hmac_key: Vec<u8>,
}

impl<S: Stream> Amp0LoggedIn<S> {
    /// List of AMP IDs
    pub fn get_amp_ids(&self) -> Result<Vec<String>, Error> {
        Ok(self
            .login_data
            .subaccounts
            .iter()
            .filter(|s| s.type_ == "2of2_no_recovery")
            .map(|s| s.gaid.to_string())
            .collect())
    }

    /// Get the next account for AMP0 account creation
    ///
    /// This must be given to [`lwk_common::Amp0Signer::amp0_account_xpub()`] to obtain the xpub to pass to
    /// [`Amp0LoggedIn::create_amp0_account()`]
    pub fn next_account(&self) -> Result<u32, Error> {
        let max_account = self
            .login_data
            .subaccounts
            .iter()
            .map(|s| s.pointer)
            .max()
            .unwrap_or(0);
        Ok(max_account + 1)
    }

    /// Create a new AMP0 account
    ///
    /// `account_xpub` must be obtained from [`lwk_common::Amp0Signer::amp0_account_xpub()`] called with the value obtained from
    /// [`Amp0LoggedIn::next_account()`]
    pub async fn create_amp0_account(
        &mut self,
        pointer: u32,
        account_xpub: &Xpub,
    ) -> Result<String, Error> {
        let amp_id = self.amp0.create_amp0_account(pointer, account_xpub).await?;
        self.login_data.subaccounts.push(GreenSubaccount {
            pointer,
            type_: "2of2_no_recovery".into(),
            gaid: amp_id.clone(),
            required_ca: 0, // TODO
        });
        self.blob.add_account_xpub(pointer, account_xpub)?;
        let blob64 = self.blob.to_base64(&self.enc_key)?;
        let hmac = compute_hmac(&self.hmac_key, &blob64)?;
        self.amp0.set_blob(&blob64, &hmac, &self.hmac).await?;
        self.hmac = hmac;
        // TODO: upload INITIAL_UPLOAD_CA
        Ok(amp_id)
    }

    /// Create a new Watch-Only entry for this wallet
    pub async fn create_watch_only(&mut self, username: &str, password: &str) -> Result<(), Error> {
        let (hashed_username, hashed_password) = encrypt_credentials(username, password);
        let wo_blob_key_hex = encrypt_blob_key(username, password, &self.enc_key)?;
        self.amp0
            .create_watch_only(&hashed_username, &hashed_password, &wo_blob_key_hex)
            .await?;
        self.blob.add_username(username)?;
        let blob64 = self.blob.to_base64(&self.enc_key)?;
        let hmac = compute_hmac(&self.hmac_key, &blob64)?;
        self.amp0.set_blob(&blob64, &hmac, &self.hmac).await?;
        self.hmac = hmac;
        Ok(())
    }
}

impl<S: Stream> Amp0Inner<S> {
    pub async fn new(stream: S) -> Result<Self, Error> {
        Ok(Self { stream })
    }

    async fn call(&self, msg: Msg) -> Result<rmpv::Value, Error> {
        let request_id = msg.request_id();
        let is_hello = matches!(msg, Msg::Hello { .. });
        let msg = serde_json::to_vec(&msg)?;
        self.stream
            .write(&msg)
            .await
            .map_err(|e| Error::Generic(format!("Failed to do call: {}", e)))?;

        // Wait for response
        let mut response_buf = vec![0u8; 10000];

        #[cfg(not(target_arch = "wasm32"))]
        let response_bytes = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.stream.read(&mut response_buf),
        )
        .await
        .map_err(|_| Error::Generic("Response timeout after 10 seconds".to_string()))?
        .map_err(|e| Error::Generic(format!("Failed to read response: {}", e)))?;

        #[cfg(target_arch = "wasm32")]
        let response_bytes = self
            .stream
            .read(&mut response_buf)
            .await
            .map_err(|e| Error::Generic(format!("Failed to read response: {}", e)))?;

        if is_hello {
            if let Ok(Msg::Welcome { .. }) = serde_json::from_slice(&response_buf[..response_bytes])
            {
                // Got a welcome response as expected
                return Ok(rmpv::Value::Nil);
            }
        }
        if let Ok(Msg::Result {
            request,
            arguments: Some(args),
            ..
        }) = serde_json::from_slice(&response_buf[..response_bytes])
        {
            if Some(request) != request_id {
                return Err(Error::Generic("Unexpected request id".to_string()));
            }
            if let [v, ..] = &args[..] {
                return Ok(v.clone());
            }
        }
        let response = String::from_utf8_lossy(&response_buf[..response_bytes]);
        Err(Error::Generic(format!("call failed, got: {}", response)))
    }

    /// Open a WAMP session
    pub async fn init_session(&self) -> Result<(), Error> {
        let mut details = WampDict::new();
        let mut roles = WampDict::new();
        let mut features = WampDict::new();
        features.insert("features".into(), Arg::Dict(WampDict::new()));
        roles.insert(ClientRole::Caller.to_str().into(), Arg::Dict(features));
        details.insert("roles".into(), Arg::Dict(roles));
        let msg = Msg::Hello {
            realm: "realm1".into(),
            details,
        };
        self.call(msg).await?;
        Ok(())
    }

    /// Login to the Green Address API with clear credentials performing the hashing internally.
    pub async fn login(
        &self,
        clear_username: &str,
        clear_password: &str,
    ) -> Result<LoginData, Error> {
        let (hashed_username, hashed_password) =
            encrypt_credentials(clear_username, clear_password);
        self.login_with_hashed_credentials(&hashed_username, &hashed_password)
            .await
    }

    /// Login to the Green Address API with pre-hashed credentials
    ///
    /// This method takes already hashed username and password. Since username and password
    /// hashing is computationally heavy (requires hundreds of milliseconds), it's recommended
    /// to use [`encrypt_credentials()`] to hash the username and password once and cache the
    /// result for subsequent logins to improve performance.
    ///
    /// For convenience, use [`Self::login`] to automatically hash clear credentials.
    pub async fn login_with_hashed_credentials(
        &self,
        hashed_username: &str,
        hashed_password: &str,
    ) -> Result<LoginData, Error> {
        self.init_session().await?;

        // Step 3: Send login call
        #[derive(Serialize)]
        struct Credentials {
            username: String,
            password: String,
            minimal: String,
        }
        let credentials = Credentials {
            username: hashed_username.into(),
            password: hashed_password.into(),
            minimal: "true".into(),
        };

        let request = WampId::generate();
        let args = vec![
            "custom".into(),
            to_value(&credentials)?,
            USER_AGENT.into(),
            true.into(),
        ];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.login.watch_only_v2".to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        let v = self.call(msg).await?;
        let login_data: LoginData = rmpv::ext::from_value(v)?;
        Ok(login_data)
    }

    /// Get the base64 encoded client blob
    pub async fn get_blob(&self) -> Result<String, Error> {
        let request = WampId::generate();
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.login.get_client_blob".to_owned(),
            arguments: Some(vec![0.into()]),
            arguments_kw: None,
        };
        let v = self.call(msg).await?;

        #[allow(unused)]
        #[derive(Deserialize)]
        struct BlobData {
            blob: String,
            hmac: String,
            sequence: u32,
        }
        let blob_data: BlobData = rmpv::ext::from_value(v)?;
        Ok(blob_data.blob)
    }

    /// Get a new address
    pub async fn get_new_address(
        &self,
        amp_subaccount: u32,
    ) -> Result<(u32, elements::Script), Error> {
        let request = WampId::generate();
        let args = vec![amp_subaccount.into(), true.into(), "p2wsh".into()];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.vault.fund".to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        let v = self.call(msg).await?;

        #[derive(Deserialize)]
        struct AddressData {
            branch: u32,
            subaccount: u32,
            pointer: u32,
            script: elements::Script,
            addr_type: String,
        }
        let data: AddressData = rmpv::ext::from_value(v)?;
        if data.branch != 1 || data.subaccount != amp_subaccount || data.addr_type != "p2wsh" {
            return Err(Error::Generic("Unexpected address data".into()));
        }
        Ok((data.pointer, data.script))
    }

    /// Upload confidential addresses
    pub async fn upload_ca(&self, amp_subaccount: u32, addresses: &[String]) -> Result<(), Error> {
        let request = WampId::generate();
        let args = vec![amp_subaccount.into(), to_value(&addresses)?];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.txs.upload_authorized_assets_confidential_address"
                .to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        // Returns true or raise an error
        let _ = self.call(msg).await?;
        Ok(())
    }

    /// Register wallet
    pub async fn register(&self, master_xpub: &Xpub, gait_path: &str) -> Result<(), Error> {
        self.init_session().await?;

        let request = WampId::generate();
        let args = vec![
            master_xpub.public_key.to_hex().into(),
            master_xpub.chain_code.to_hex().into(),
            USER_AGENT.into(),
            gait_path.into(),
        ];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.login.register".to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        // Returns true or raise an error
        let _ = self.call(msg).await?;
        Ok(())
    }

    /// Get challenge
    pub async fn get_challenge(&self, login_address: &Address) -> Result<String, Error> {
        let request = WampId::generate();
        let hw_nlocktime_support = true;
        let args = vec![
            login_address.to_string().into(),
            hw_nlocktime_support.into(),
        ];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.login.get_trezor_challenge".to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        let v = self.call(msg).await?;
        let challenge: String = rmpv::ext::from_value(v)?;
        Ok(challenge)
    }

    /// Authenticate
    pub async fn authenticate(&self, sig: &str) -> Result<LoginData, Error> {
        let request = WampId::generate();
        let args = vec![
            sig.into(),
            true.into(), // minimal
            "GA".into(), // path hex
            "".into(),   // device id
            USER_AGENT.into(),
        ];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.login.authenticate".to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        let v = self.call(msg).await?;
        let login_data: LoginData = rmpv::ext::from_value(v)?;
        Ok(login_data)
    }

    /// Set blob
    pub async fn set_blob(
        &self,
        blob64: &str,
        hmac: &str,
        previous_hmac: &str,
    ) -> Result<(), Error> {
        let request = WampId::generate();
        let args = vec![
            blob64.into(),
            0.into(), // sequence
            hmac.into(),
            previous_hmac.into(),
        ];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.login.set_client_blob".to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        // Returns true or raise an error
        let _ = self.call(msg).await?;
        Ok(())
    }

    /// Create subaccount
    pub async fn create_amp0_account(
        &self,
        pointer: u32,
        subaccount_xpub: &Xpub,
    ) -> Result<String, Error> {
        let request = WampId::generate();
        let args = vec![
            pointer.into(),
            "".into(),                         // name
            "2of2_no_recovery".into(),         // type
            to_value(&vec![subaccount_xpub])?, // xpubs
            to_value(&vec!["".to_string()])?,  // sigs
        ];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.txs.create_subaccount_v2".to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        let v = self.call(msg).await?;
        let amp_id: String = rmpv::ext::from_value(v)?;
        Ok(amp_id)
    }

    /// Create watch only
    pub async fn create_watch_only(
        &self,
        hashed_username: &str,
        hashed_password: &str,
        wo_blob_key_hex: &str,
    ) -> Result<(), Error> {
        let request = WampId::generate();
        let args = vec![
            hashed_username.into(),
            hashed_password.into(),
            wo_blob_key_hex.into(),
        ];
        let msg = Msg::Call {
            request,
            options: WampDict::new(),
            procedure: "com.greenaddress.addressbook.sync_custom".to_owned(),
            arguments: Some(args),
            arguments_kw: None,
        };
        // Returns true or raise an error
        let _ = self.call(msg).await?;
        Ok(())
    }
}

fn get_entropy(username: &str, password: &str) -> [u8; 64] {
    // https://gl.blockstream.io/blockstream/green/gdk/-/blame/master/src/utils.cpp#L334
    let salt_string: &[u8] = &WATCH_ONLY_SALT;

    let u_p = format!("{}{}", username, password);
    let mut entropy = vec![0u8; 4 + u_p.len()];

    // Write username length as 32-bit integer
    let username_len = username.len() as u32;
    entropy[0..4].copy_from_slice(&username_len.to_le_bytes());

    // Copy concatenated username and password
    entropy[4..].copy_from_slice(u_p.as_bytes());

    let mut output = [0u8; 64];
    let params = Params::new(
        14, // log_n (2^14 = 16384 iterations)
        8,  // r (block size)
        8,  // p (parallelization)
        64, // output length in bytes
    );
    scrypt(
        &entropy,
        salt_string,
        &params.expect("script parameters defined statically"),
        &mut output,
    )
    .expect("`output.len() > 0 && output.len() <= (2^32 - 1) * 32`.");

    output
}

fn encrypt_credentials(username: &str, password: &str) -> (String, String) {
    let entropy = get_entropy(username, password);

    // https://gl.blockstream.io/blockstream/green/gdk/-/blame/master/src/ga_session.cpp#L222

    // Calculate u_blob and p_blob using PBKDF2-HMAC-SHA512-256
    let mut u_blob = [0u8; 32];
    let mut p_blob = [0u8; 32];

    let _ = pbkdf2::<Hmac<Sha512>>(&entropy, &WO_SEED_U, 2048, &mut u_blob);
    let _ = pbkdf2::<Hmac<Sha512>>(&entropy, &WO_SEED_P, 2048, &mut p_blob);

    (hex::_encode(&u_blob), hex::_encode(&p_blob))
}

fn decrypt_blob_key(
    username: &str,
    password: &str,
    wo_blob_key_hex: &str,
) -> Result<Vec<u8>, Error> {
    let entropy = get_entropy(username, password);
    let mut wo_aes_key = [0u8; 32];
    let _ = pbkdf2::<Hmac<Sha512>>(&entropy, &WO_SEED_K, 2048, &mut wo_aes_key);

    let data = hex::_decode(wo_blob_key_hex)?;

    let iv: [u8; 16] = data[..16]
        .try_into()
        .map_err(|_| Error::Generic("Invalid IV".to_string()))?;
    let enc_key = cbc::Decryptor::<aes::Aes256>::new(&wo_aes_key.into(), &iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(&data[16..])
        .map_err(|e| Error::Generic(e.to_string()))?;

    Ok(enc_key)
}

#[allow(deprecated)]
fn blob_cipher(enc_key: &[u8]) -> Result<Aes256Gcm, Error> {
    if enc_key.len() != 32 {
        return Err(Error::Generic("Invalid encryption key length".into()));
    }
    // panicks on length mismatch
    use aes_gcm::KeyInit;
    let key = Key::<Aes256Gcm>::from_slice(enc_key);
    Ok(Aes256Gcm::new(key))
}

#[allow(deprecated)]
fn decrypt_blob(enc_key: &[u8], blob64: &str) -> Result<Vec<u8>, Error> {
    let wo_blob = BASE64_STANDARD
        .decode(blob64)
        .map_err(|e| Error::Generic(e.to_string()))?;

    let cipher = blob_cipher(enc_key)?;

    let nonce: [u8; 12] = wo_blob[..12]
        .try_into()
        .map_err(|_| Error::Generic("Invalid nonce".to_string()))?;
    let nonce = Nonce::from_slice(&nonce);
    let plaintext = cipher.decrypt(nonce, &wo_blob[12..])?;
    // plaintext should start with [1, 0, 0, 0] but it's not worth checking it here
    // as it might break after if someone sets the blob without this prefix
    Ok(plaintext)
}

fn encrypt_blob(enc_key: &[u8], plaintext: &[u8]) -> Result<String, Error> {
    let cipher = blob_cipher(enc_key)?;

    use aes_gcm::aead::{AeadCore, OsRng};
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher.encrypt(&nonce, plaintext)?;
    let mut wo_blob = nonce.to_vec();
    wo_blob.extend(ciphertext);
    Ok(BASE64_STANDARD.encode(&wo_blob))
}

fn parse_value(blob: &[u8]) -> Result<rmpv::Value, Error> {
    // decompress
    // bytes 0 to 4 are prefix
    // bytes 4 to 8 are ignored
    let mut d = ZlibDecoder::new(&blob[8..]);
    let mut v = vec![];
    d.read_to_end(&mut v)?;

    // messagePack to json
    let mut cursor = std::io::Cursor::new(v);
    let value = rmpv::decode::read_value(&mut cursor)?;
    Ok(value)
}

fn from_value(value: &rmpv::Value) -> Result<Vec<u8>, Error> {
    // json to messagePack
    let mut v = Vec::new();
    rmpv::encode::write_value(&mut v, value)?;
    let bytes_len = v.len() as u32;

    // compress
    use flate2::{read::ZlibEncoder, Compression};
    let cursor = std::io::Cursor::new(v);
    let mut z = ZlibEncoder::new(cursor, Compression::best());
    let mut compressed = Vec::new();
    compressed.extend(vec![1, 0, 0, 0]);
    compressed.extend(bytes_len.to_le_bytes());
    z.read_to_end(&mut compressed)?;

    Ok(compressed)
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Blob {
    version: u32,
    sa_names: Option<BTreeMap<String, String>>,
    tx_memos: Option<rmpv::Value>,
    sa_hidden: Option<rmpv::Value>,
    slip77key: BTreeMap<String, String>,
    watchonly: BTreeMap<String, rmpv::Value>,
    // Other reserved values
    _07: Option<rmpv::Value>,
    _08: Option<rmpv::Value>,
    _09: Option<rmpv::Value>,
    _10: Option<rmpv::Value>,
    _11: Option<rmpv::Value>,
    _12: Option<rmpv::Value>,
    _13: Option<rmpv::Value>,
    _14: Option<rmpv::Value>,
    _15: Option<rmpv::Value>,
    _16: Option<rmpv::Value>,
    _17: Option<rmpv::Value>,
    _18: Option<rmpv::Value>,
    _19: Option<rmpv::Value>,
    _20: Option<rmpv::Value>,
    _21: Option<rmpv::Value>,
    _22: Option<rmpv::Value>,
    _23: Option<rmpv::Value>,
    _24: Option<rmpv::Value>,
    _25: Option<rmpv::Value>,
    _26: Option<rmpv::Value>,
    _27: Option<rmpv::Value>,
    _28: Option<rmpv::Value>,
    _29: Option<rmpv::Value>,
    _30: Option<rmpv::Value>,
    _31: Option<rmpv::Value>,
    _32: Option<rmpv::Value>,

    #[serde(skip)]
    xpubs: HashMap<Xpub, Vec<u32>>,
    #[serde(skip)]
    slip77_key: String,
}

impl Blob {
    /// Create a new blob
    fn new(signer_data: &Amp0SignerData) -> Result<Self, Error> {
        let mut slip77key = BTreeMap::new();
        let slip77_str = signer_data.slip77_key().to_string();
        slip77key.insert("key".into(), slip77_str);
        let mut watchonly = BTreeMap::new();
        let mut xpubs = BTreeMap::new();
        xpubs.insert(signer_data.master_xpub().to_string(), vec![]);
        // TODO: use const
        xpubs.insert(signer_data.login_xpub().to_string(), vec![1195487518u32]);
        xpubs.insert(
            signer_data.client_secret_xpub().to_string(),
            vec![4032918387],
        );
        watchonly.insert("xpubs".into(), to_value(&xpubs)?);
        watchonly.insert("username".into(), "".into());
        let mut blob = Self {
            version: 4,
            slip77key,
            watchonly,
            ..Default::default()
        };
        blob.set_fields()?;
        Ok(blob)
    }

    #[allow(unused)]
    fn from_value(value: &rmpv::Value) -> Result<Self, Error> {
        let mut blob: Self = rmpv::ext::from_value(value.clone())?;
        blob.set_fields()?;
        Ok(blob)
    }

    fn from_base64(blob64: &str, enc_key: &[u8]) -> Result<Self, Error> {
        let plaintext = decrypt_blob(enc_key, blob64)?;
        let value = parse_value(&plaintext)?;
        let mut blob: Self = rmpv::ext::from_value(value)?;
        blob.set_fields()?;
        Ok(blob)
    }

    fn set_fields(&mut self) -> Result<(), Error> {
        let slip77_key = self
            .slip77key
            .get("key")
            .ok_or_else(|| Error::Generic("Unexpected value".into()))?;
        self.slip77_key = slip77_key[(slip77_key.len() - 64)..].to_string();

        let xpubs = self
            .watchonly
            .get("xpubs")
            .ok_or_else(|| Error::Generic("Unexpected value".into()))?;
        self.xpubs = rmpv::ext::from_value(xpubs.clone())?;

        Ok(())
    }

    #[allow(unused)]
    fn to_value(&self) -> Result<rmpv::Value, Error> {
        Ok(rmpv::ext::to_value(self)?)
    }

    fn to_base64(&self, enc_key: &[u8]) -> Result<String, Error> {
        let value = rmpv::ext::to_value(self)?;
        let plaintext = from_value(&value)?;
        let blob64 = encrypt_blob(enc_key, &plaintext)?;
        Ok(blob64)
    }

    fn find_xpub(&self, amp_subaccount: u32) -> Option<Xpub> {
        for (k, v) in &self.xpubs {
            if let [cn1, cn2] = v[..] {
                if cn1 == (3 + 2u32.pow(31)) && cn2 == (amp_subaccount + 2u32.pow(31)) {
                    return Some(*k);
                }
            }
        }
        None
    }

    fn find_master_xpub(&self) -> Option<Xpub> {
        for (k, v) in &self.xpubs {
            if v.is_empty() {
                return Some(*k);
            }
        }
        None
    }

    fn add_account_xpub(&mut self, pointer: u32, account_xpub: &Xpub) -> Result<(), Error> {
        use rmpv::Value;
        let path = vec![0x80000000 + 3, 0x80000000 + pointer];

        if self.xpubs.keys().all(|xpub| {
            xpub.public_key != account_xpub.public_key && xpub.chain_code != account_xpub.chain_code
        }) {
            // Account xpub is not in the blob
            // Insert xpub in (de)serialized struct
            if let Some(Value::Map(ref mut xpubs)) = self.watchonly.get_mut("xpubs") {
                let v: Vec<Value> = path.clone().into_iter().map(Value::from).collect();
                xpubs.push((Value::from(account_xpub.to_string()), Value::Array(v)));
            } else {
                return Err(Error::Generic("Unexpected value".into()));
            }

            // Insert xpub in parsed struct
            self.xpubs.insert(*account_xpub, path);
        }

        Ok(())
    }

    fn add_username(&mut self, username: &str) -> Result<(), Error> {
        self.watchonly.insert("username".into(), username.into());
        Ok(())
    }
}

fn server_master_xpub(network: &Network) -> Xpub {
    // Values from GDK
    let (public_key, chain_code, network_kind) = match network {
        Network::Liquid => (
            "02c408c3bb8a3d526103fb93246f54897bdd997904d3e18295b49a26965cb41b7f",
            "02721cc509aa0c2f4a90628e9da0391b196abeabc6393ed4789dd6222c43c489",
            NetworkKind::Main,
        ),
        Network::TestnetLiquid => (
            "02c47d84a5b256ee3c29df89642d14b6ed73d17a2b8af0aca18f6f1900f1633533",
            "c660eec6d9c536f4121854146da22e02d4c91d72af004d41729b9a592f0788e5",
            NetworkKind::Test,
        ),
        Network::LocaltestLiquid => (
            "036307e560072ed6ce0aa5465534fb5c258a2ccfbc257f369e8e7a181b16d897b3",
            "b60befcc619bb1c212732770fe181f2f1aa824ab89f8aab49f2e13e3a56f0f04",
            NetworkKind::Test,
        ),
    };
    let public_key = PublicKey::from_str(public_key).expect("hardcoded");
    let chain_code = ChainCode::from_str(chain_code).expect("hardcoded");

    Xpub {
        network: network_kind,
        depth: 0,
        parent_fingerprint: Fingerprint::default(),
        child_number: ChildNumber::Normal { index: 0 },
        public_key,
        chain_code,
    }
}

fn derive_server_xpub(
    network: &Network,
    gait_path: &str,
    amp_subaccount: u32,
) -> Result<String, Error> {
    let xpub = server_master_xpub(network);
    let fingerprint = xpub.fingerprint();
    let gait_path_bytes = hex::_decode(gait_path)?;
    let gait_path: Vec<_> = gait_path_bytes
        .chunks(2)
        .map(|chunk| u32::from_be_bytes([0, 0, chunk[0], chunk[1]]).to_string())
        .collect();

    let gait_path = gait_path.join("/");
    let server_path = format!("3/{gait_path}/{amp_subaccount}");

    let derivation_path = DerivationPath::from_str(&server_path)?;
    let derived_xpub = xpub.derive_pub(&EC, &derivation_path)?;

    Ok(format!("[{fingerprint}/{server_path}]{derived_xpub}"))
}

fn amp_descriptor(
    blob: &Blob,
    amp_subaccount: u32,
    network: &Network,
    gait_path: &str,
) -> Result<String, Error> {
    let server_xpub = derive_server_xpub(network, gait_path, amp_subaccount)?;

    let master_xpub = blob
        .find_master_xpub()
        .ok_or_else(|| Error::Generic("Missing master xpub".into()))?;
    let fingerprint = master_xpub.fingerprint();
    let user_keyorigin = format!("[{fingerprint}/3h/{amp_subaccount}h]");
    // TODO: improve error
    let user_xpub = blob
        .find_xpub(amp_subaccount)
        .ok_or_else(|| Error::Generic("Invalid AMP subaccount".into()))?;
    let slip77_key = &blob.slip77_key;
    let desc = format!("ct(slip77({slip77_key}),elsh(wsh(multi(2,{server_xpub}/*,{user_keyorigin}{user_xpub}/1/*))))");
    Ok(desc)
}

/// Default URL for Green Backend
pub fn default_url(network: Network) -> Result<&'static str, Error> {
    match network {
        Network::Liquid => Ok("wss://green-liquid-mainnet.blockstream.com/v2/ws/"),
        Network::TestnetLiquid => Ok("wss://green-liquid-testnet.blockstream.com/v2/ws/"),
        Network::LocaltestLiquid => Ok("ws://localhost:8080/v2/ws"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn stream_with_network(network: Network) -> Result<WebSocketClient, Error> {
    let url = default_url(network)?;
    let stream = WebSocketClient::connect_wamp(url)
        .await
        .map_err(|e| Error::Generic(e.to_string()))?;
    Ok(stream)
}

#[cfg(not(target_arch = "wasm32"))]
impl Amp0Inner<WebSocketClient> {
    #[allow(unused)]
    async fn with_network(network: Network) -> Result<Self, Error> {
        let stream = stream_with_network(network).await?;
        Ok(Self { stream })
    }
}

/// WebSocket client for non-WASM environments using tokio-tungstenite
#[cfg(not(target_arch = "wasm32"))]
pub struct WebSocketClient {
    write_stream: Arc<
        Mutex<
            futures::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                tokio_tungstenite::tungstenite::Message,
            >,
        >,
    >,
    read_stream: Arc<
        Mutex<
            futures::stream::SplitStream<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
            >,
        >,
    >,
}

#[cfg(not(target_arch = "wasm32"))]
impl WebSocketClient {
    /// Connect to a WebSocket URL
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use futures::StreamExt;
        use tokio_tungstenite::connect_async;

        let (ws_stream, _) = connect_async(url).await?;
        let (write, read) = ws_stream.split();

        Ok(Self {
            write_stream: Arc::new(Mutex::new(write)),
            read_stream: Arc::new(Mutex::new(read)),
        })
    }

    /// Connect to a WebSocket URL with a specific protocol
    pub async fn connect_with_protocol(
        url: &str,
        protocol: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use futures::StreamExt;
        use tokio_tungstenite::{
            connect_async_with_config,
            tungstenite::{client::IntoClientRequest, http::HeaderValue},
        };

        // Start with the URL and let it create the base request
        let mut request = url.into_client_request()?;

        // Add the protocol header
        request
            .headers_mut()
            .insert("Sec-WebSocket-Protocol", HeaderValue::from_str(protocol)?);

        let (ws_stream, _) = connect_async_with_config(request, None, false).await?;
        let (write, read) = ws_stream.split();

        Ok(Self {
            write_stream: Arc::new(Mutex::new(write)),
            read_stream: Arc::new(Mutex::new(read)),
        })
    }

    /// Connect to a WebSocket URL with WAMP 2.0 JSON protocol
    pub async fn connect_wamp(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::connect_with_protocol(url, "wamp.2.json").await
    }

    /// Send a text message
    pub async fn send_text(
        &self,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let mut write_stream = self.write_stream.lock().await;
        write_stream.send(Message::Text(text.to_string())).await?;
        Ok(())
    }

    /// Send binary data
    pub async fn send_binary(
        &self,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let mut write_stream = self.write_stream.lock().await;
        write_stream.send(Message::Binary(data.to_vec())).await?;
        Ok(())
    }
}

/// Custom error type for WebSocket operations
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum WebSocketError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Invalid message type")]
    InvalidMessageType,
}

/// Implement the Stream trait for WebSocketClient
#[cfg(not(target_arch = "wasm32"))]
impl Stream for WebSocketClient {
    type Error = WebSocketError;

    async fn read(&self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        use futures::StreamExt;
        use tokio_tungstenite::tungstenite::Message;

        let mut read_stream = self.read_stream.lock().await;

        loop {
            match read_stream.next().await {
                Some(Ok(Message::Binary(data))) => {
                    let len = std::cmp::min(buf.len(), data.len());
                    buf[..len].copy_from_slice(&data[..len]);
                    return Ok(len);
                }
                Some(Ok(Message::Text(text))) => {
                    let data = text.as_bytes();
                    let len = std::cmp::min(buf.len(), data.len());
                    buf[..len].copy_from_slice(&data[..len]);
                    return Ok(len);
                }
                Some(Ok(Message::Close(_))) => {
                    return Err(WebSocketError::ConnectionClosed);
                }
                Some(Ok(_)) => {
                    // Skip ping/pong frames
                    continue;
                }
                Some(Err(e)) => {
                    return Err(WebSocketError::WebSocket(e));
                }
                None => {
                    return Err(WebSocketError::ConnectionClosed);
                }
            }
        }
    }

    async fn write(&self, data: &[u8]) -> Result<(), Self::Error> {
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let mut write_stream = self.write_stream.lock().await;

        // Try to send as text first (for JSON protocols), fall back to binary
        if let Ok(text) = std::str::from_utf8(data) {
            write_stream.send(Message::Text(text.to_string())).await?;
        } else {
            write_stream.send(Message::Binary(data.to_vec())).await?;
        }
        Ok(())
    }
}

/// Amp0 blocking module
#[cfg(not(target_arch = "wasm32"))]
pub mod blocking {
    use super::*;
    use tokio::runtime::Runtime;

    /// Blocking version of [`super::Amp0`]
    pub struct Amp0 {
        rt: Runtime,
        inner: super::Amp0<super::WebSocketClient>,
    }

    impl Amp0 {
        /// Index of the last returned address
        pub fn last_index(&self) -> u32 {
            self.inner.last_index
        }

        /// AMP identifier
        pub fn amp_id(&self) -> &str {
            &self.inner.amp_id
        }

        /// The LWK watch-only wallet descriptor corresponding to the AMP0 (sub)account.
        ///
        /// <div class="warning">
        /// <b>WARNING:</b>
        ///
        /// Do not derive addresses using [`WolletDescriptor::address()`] or [`crate::Wollet::address()`].
        ///
        /// See [`Amp0`] for more details.
        /// </div>
        pub fn wollet_descriptor(&self) -> WolletDescriptor {
            self.inner.wollet_descriptor()
        }

        /// Create a new AMP0 context
        pub fn new(
            network: Network,
            username: &str,
            password: &str,
            amp_id: &str,
        ) -> Result<Self, Error> {
            let rt = Runtime::new()?;
            let inner = rt.block_on(super::Amp0::<WebSocketClient>::new_with_network(
                network, username, password, amp_id,
            ))?;
            Ok(Amp0 { rt, inner })
        }

        /// Get an address
        ///
        /// If `index` is None, a new address is returned.
        pub fn address(&mut self, index: Option<u32>) -> Result<AddressResult, Error> {
            self.rt.block_on(self.inner.address(index))
        }

        /// Ask AMP0 server to cosign
        pub fn sign(&self, pset: &Amp0Pset) -> Result<Transaction, Error> {
            self.rt.block_on(self.inner.sign(pset))
        }
    }

    /// Blocking version of [`super::Amp0Connected`]
    pub struct Amp0Connected {
        rt: Runtime,
        inner: super::Amp0Connected<super::WebSocketClient>,
    }

    /// Blocking version of [`super::Amp0LoggedIn`]
    pub struct Amp0LoggedIn {
        rt: Runtime,
        inner: super::Amp0LoggedIn<super::WebSocketClient>,
    }

    impl Amp0Connected {
        /// Connect and register to AMP0
        pub fn new(network: Network, signer_data: super::Amp0SignerData) -> Result<Self, Error> {
            let rt = Runtime::new()?;
            let inner = rt.block_on(super::Amp0Connected::<WebSocketClient>::new_(
                network,
                signer_data,
            ))?;
            Ok(Amp0Connected { rt, inner })
        }

        /// Obtain a login challenge
        ///
        /// This must be signed with [`lwk_common::Amp0Signer::amp0_sign_challenge()`].
        pub fn get_challenge(&self) -> Result<String, Error> {
            self.rt.block_on(self.inner.get_challenge())
        }

        /// Log in
        ///
        /// `sig` must be obtained from [`lwk_common::Amp0Signer::amp0_sign_challenge()`] called with the value returned
        /// by [`Amp0Connected::get_challenge()`]
        pub fn login(self, sig: &str) -> Result<Amp0LoggedIn, Error> {
            let amp0loggedin = self.rt.block_on(self.inner.login(sig))?;
            Ok(Amp0LoggedIn {
                rt: self.rt,
                inner: amp0loggedin,
            })
        }
    }

    impl Amp0LoggedIn {
        /// List of AMP IDs.
        pub fn get_amp_ids(&self) -> Result<Vec<String>, Error> {
            self.inner.get_amp_ids()
        }

        /// Get the next account for AMP0 account creation
        ///
        /// This must be given to [`lwk_common::Amp0Signer::amp0_account_xpub()`] to obtain the xpub to pass to
        /// [`Amp0LoggedIn::create_amp0_account()`]
        pub fn next_account(&self) -> Result<u32, Error> {
            self.inner.next_account()
        }

        /// Create a new AMP0 account
        ///
        /// `account_xpub` must be obtained from [`lwk_common::Amp0Signer::amp0_account_xpub()`] called with the value obtained from
        /// [`Amp0LoggedIn::next_account()`]
        pub fn create_amp0_account(
            &mut self,
            pointer: u32,
            account_xpub: &Xpub,
        ) -> Result<String, Error> {
            self.rt
                .block_on(self.inner.create_amp0_account(pointer, account_xpub))
        }

        /// Create a new Watch-Only entry for this wallet
        pub fn create_watch_only(&mut self, username: &str, password: &str) -> Result<(), Error> {
            self.rt
                .block_on(self.inner.create_watch_only(username, password))
        }
    }
}

/// A PSET to use with AMP0
///
/// When asking AMP0 to cosign, the caller must pass some extra data that does not belong to the
/// PSET. This struct holds and manage the necessary data.
///
/// If you're not dealing with AMP0, do not use this struct.
pub struct Amp0Pset {
    pset: PartiallySignedTransaction,
    blinding_nonces: Vec<String>,
}

impl Amp0Pset {
    /// Construct a PSET to use with AMP0
    pub fn new(
        pset: PartiallySignedTransaction,
        blinding_nonces: Vec<String>,
    ) -> Result<Self, Error> {
        if pset.n_outputs() != blinding_nonces.len() {
            return Err(Error::Generic("Invalid blinding nonces".into()));
        }
        for (idx, output) in pset.outputs().iter().enumerate() {
            let txout = output.to_txout();
            if txout.is_partially_blinded() {
                let shared_secret = SecretKey::from_str(&blinding_nonces[idx])?;
                let txoutsecrets = unblind_with_shared_secret(&txout, shared_secret)?;
                let asset_comm = Generator::new_blinded(
                    &EC,
                    txoutsecrets.asset.into_inner().to_byte_array().into(),
                    txoutsecrets.asset_bf.into_inner(),
                );
                let amount_comm = PedersenCommitment::new(
                    &EC,
                    txoutsecrets.value,
                    txoutsecrets.value_bf.into_inner(),
                    asset_comm,
                );
                if output.amount != Some(txoutsecrets.value)
                    || output.asset != Some(txoutsecrets.asset)
                    || output.amount_comm != Some(amount_comm)
                    || output.asset_comm != Some(asset_comm)
                {
                    return Err(Error::Generic("Invalid blinding nonce".into()));
                }
            } else if !blinding_nonces[idx].is_empty() {
                return Err(Error::Generic("Invalid blinding nonce".into()));
            }
        }
        Ok(Self {
            pset,
            blinding_nonces,
        })
    }

    /// Get the PSET
    pub fn pset(&self) -> &PartiallySignedTransaction {
        &self.pset
    }

    /// Get the blinding nonces
    pub fn blinding_nonces(&self) -> &[String] {
        &self.blinding_nonces
    }
}

fn unblind_with_shared_secret(
    txout: &TxOut,
    shared_secret: SecretKey,
) -> Result<TxOutSecrets, Error> {
    let commitment = txout
        .value
        .commitment()
        .ok_or_else(|| Error::Generic("Missing value commitment".into()))?;
    let additional_generator = txout
        .asset
        .commitment()
        .ok_or_else(|| Error::Generic("Missing asset commitment".into()))?;
    let rangeproof = txout
        .witness
        .rangeproof
        .as_ref()
        .ok_or_else(|| Error::Generic("Missing rangeproof".into()))?;

    let (opening, _) = rangeproof.rewind(
        &EC,
        commitment,
        shared_secret,
        txout.script_pubkey.as_bytes(),
        additional_generator,
    )?;

    let (asset, asset_bf) = opening.message.as_ref().split_at(32);
    let asset = AssetId::from_slice(asset)?;
    let asset_bf = AssetBlindingFactor::from_slice(&asset_bf[..32])?;

    let value = opening.value;
    let value_bf = ValueBlindingFactor::from_slice(&opening.blinding_factor[..32])?;

    Ok(TxOutSecrets {
        asset,
        asset_bf,
        value,
        value_bf,
    })
}

fn derive_gait_path(xpub: &Xpub) -> String {
    // expected xpub is m/18241h
    // chaincode + pubkey;
    let mut input: Vec<u8> = vec![];
    input.extend(xpub.chain_code.as_bytes());
    input.extend(xpub.public_key.serialize());

    let ga_key = b"GreenAddress.it HD wallet path";
    use hmac::Mac;
    let mut mac = Hmac::<Sha512>::new_from_slice(ga_key).expect("HMAC can take key of any size");
    mac.update(&input);
    let gait_path_bytes = mac.finalize().into_bytes();

    hex::_encode(&gait_path_bytes)
}

fn derive_blob_keys(client_secret_xpub: &Xpub) -> (Vec<u8>, Vec<u8>) {
    let mut tmp_key = [0u8; 64];

    let pubkey = client_secret_xpub.public_key.serialize();
    let _ = pbkdf2::<Hmac<Sha512>>(&pubkey, &BLOB_SALT, 2048, &mut tmp_key);

    let enc_key = sha256::Hash::hash(&tmp_key[32..]).to_byte_array().to_vec();
    let hmac_key = tmp_key[32..].to_vec();
    (enc_key, hmac_key)
}

fn compute_hmac(hmac_key: &[u8], blob64: &str) -> Result<String, Error> {
    let blob = BASE64_STANDARD
        .decode(blob64)
        .map_err(|e| Error::Generic(e.to_string()))?;
    use hmac::Mac;
    let mut mac = Hmac::<Sha256>::new_from_slice(hmac_key).expect("HMAC can take key of any size");
    mac.update(&blob);
    let hmac_bytes = mac.finalize().into_bytes();
    Ok(BASE64_STANDARD.encode(hmac_bytes))
}

fn encrypt_blob_key(username: &str, password: &str, enc_key: &[u8]) -> Result<String, Error> {
    let entropy = get_entropy(username, password);
    let mut wo_aes_key = [0u8; 32];
    let _ = pbkdf2::<Hmac<Sha512>>(&entropy, &WO_SEED_K, 2048, &mut wo_aes_key);

    let mut iv = [0u8; 16];
    let mut rng = thread_rng();
    rng.fill_bytes(&mut iv);
    let cyphertext = cbc::Encryptor::<aes::Aes256>::new(&wo_aes_key.into(), (&iv).into())
        .encrypt_padded_vec_mut::<Pkcs7>(enc_key);

    let mut blob_key = iv.to_vec();
    blob_key.extend(cyphertext);
    Ok(hex::_encode(&blob_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test WebSocket connection to Blockstream's Green Liquid mainnet endpoint
    /// This test demonstrates connecting to a real WebSocket server with WAMP protocol
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    #[ignore] // Requires network connectivity
    async fn test_websocket_client_wamp_connection() {
        let client =
            WebSocketClient::connect_wamp("wss://green-liquid-mainnet.blockstream.com/v2/ws/")
                .await
                .expect("Failed to connect to WebSocket");

        // WAMP HELLO message
        let hello_message = r#"[1, "realm1", {"roles": {"caller": {"features": {}}}}]"#;

        // Send HELLO message using Stream trait
        let hello_bytes = hello_message.as_bytes();
        client
            .write(hello_bytes)
            .await
            .expect("Failed to send HELLO message");

        // Read response using Stream trait
        let mut response_buffer = vec![0u8; 4096];
        let bytes_read = client
            .read(&mut response_buffer)
            .await
            .expect("Failed to read response");

        // Convert response to string and verify it's a WELCOME message
        let response_str = String::from_utf8_lossy(&response_buffer[..bytes_read]);

        // Parse as JSON and verify structure
        let response_json: serde_json::Value =
            serde_json::from_str(&response_str).expect("Failed to parse response as JSON");

        if let serde_json::Value::Array(ref arr) = response_json {
            assert!(
                arr.len() >= 3,
                "WELCOME message should have at least 3 elements"
            );
            assert_eq!(
                arr[0], 2,
                "First element should be 2 (WELCOME message type)"
            );
            assert!(arr[1].is_number(), "Second element should be session ID");
            assert!(arr[2].is_object(), "Third element should be details object");
        } else {
            panic!("Response should be a JSON array");
        }
    }

    /// Test WebSocket client creation (without network)
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn test_websocket_client_creation() {
        // This test will fail since the URL doesn't exist, but it tests the API
        let result = WebSocketClient::connect("ws://localhost:1234").await;
        assert!(
            result.is_err(),
            "Connection should fail for non-existent URL"
        );
    }

    /// Test Amp0Inner login functionality with proper WAMP protocol flow
    /// This test demonstrates the complete WAMP handshake + login flow
    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    #[ignore] // Requires network connectivity
    async fn test_amp0_fail_login() {
        let amp0 = Amp0Inner::with_network(Network::Liquid)
            .await
            .expect("Failed to connect to WebSocket");

        // Test with invalid credentials - should get proper error response, not timeout
        let response = amp0
            .login("invalid-user", "invalid-password")
            .await
            .unwrap_err();

        // Should get an error response like: [8,48,1,{},"com.greenaddress.error",["http://greenaddressit.com/error#usernotfound","User not found or invalid password",{}]]
        let response_str = format!("{:?}", response);
        assert!(!response_str.is_empty(), "Response should not be empty");
        assert!(
            response_str.contains("com.greenaddress.error") || response_str.contains("error"),
            "Response should contain error information, got: {}",
            response_str
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    #[ignore] // Requires network connectivity
    async fn test_amp0_ok_login() {
        let amp0 = Amp0Inner::with_network(Network::Liquid)
            .await
            .expect("Failed to connect to WebSocket");

        let response = amp0
            .login("userleo456", "userleo456")
            .await
            .expect("Should get a response (even if it's an error)");

        assert_eq!(response.gait_path.len(), 128);
        assert_eq!(response.wo_blob_key.unwrap().len(), 128);
        assert_eq!(response.subaccounts.len(), 1);
        assert_eq!(response.subaccounts[0].type_, "2of2_no_recovery");
        assert_eq!(response.subaccounts[0].pointer, 1);
        assert_eq!(
            response.subaccounts[0].gaid,
            "GA2zxWdhAYtREeYCVFTGRhHQmYMPAP"
        );

        let _blob = amp0.get_blob().await.unwrap();

        let amp_subaccount = 1;
        let (_pointer, _script) = amp0.get_new_address(amp_subaccount).await.unwrap();
    }

    #[test]
    fn test_slow_amp0_login_utils() {
        let (encrypted_username, encrypted_password) =
            encrypt_credentials("userleo456", "userleo456");

        assert_eq!(
            encrypted_username,
            "a3c7f7de9a34bcab4554f7cedf6046e041eeb3a9211466d92ecaa9763ac3557b"
        );
        assert_eq!(
            encrypted_password,
            "f3ac0f33fe97412a39ebb5d11d111961a754ecbbbdf12c71342adb7022ae3a2d"
        );
    }

    #[test]
    fn test_slow_amp0_decrypt_blob() {
        use elements::hashes::hex::DisplayHex;

        // Target value to match
        let expected_descriptor = "ct(slip77(8280c0855f6e79fcce8712ddee830f04b6f75fc03ffc771a49d71499cce148b6),elsh(wsh(multi(2,[0557d83a/3/3320/60546/15157/41212/14985/38799/25816/12131/13561/54922/2852/56496/53096/60883/33605/54091/38661/40920/32654/56040/43253/45144/11278/64888/46277/8839/7065/20066/31815/30779/10369/43255/1]xpub7DNqsKDE71pikazVtZgBqxccgbcYrmUmURBcg8uZuf7wEvUxkZeAHEgSQ3GMMmpkVRWru4cu5QDkWxqaokEjRpcxGw6Rust4nBz7UH1NGPq/*,[aea13085/3h/1h]xpub69mdgvyMbhUaD7XFqmjNfo7RdCnW2w1xfEmNn7DV5XYqwPSKkcgMtqQ7T776MCBNXWZrkqwx6NArqE34WCBW86CdMgLesYtnvjSaLSMy2Xc/1/*))))";

        // Watch-only credentials
        let username = "userleo456";
        let password = "userleo456";

        // Values from login data
        let amp_subaccount = 1;
        let network = Network::Liquid;
        let gait_path = "0cf8ec823b35a0fc3a89978f64d82f6334f9d68a0b24dcb0cf68edd38345d34b97059fd87f8edae8a8f5b0582c0efd78b4c522871b994e627c47783b2881a8f7";
        let wo_blob_key_hex = "e55785016af0cf58e2c4fc735ec16f460afe7c5138b335455b4fea7ec1fa1fe4066930e67aed687fef8c1f418ee6e43c7e29a37bed8551a36e1456d9a3b24621";

        // Values from get client blob
        let wo_blob = "rVxR0vu5UkNE5cDwSXQxlhpe52TM+02SJ66v9n4KgVHCNAYhNSsaubJMQMJKKx6BVaS/T8NnVeb+O5JF2zq/eEMC62+dLClpSAo28U5El0DhvkcunYVcWhUZ9kZido0tiJeDYMi3ZNuPgUlqRW6vrWsPcQ+2165Ti9Pt7MJEdrzgLblilszOEeWofatwdlJKyO4yB0LrnxDErSyQ18Zok9KRqjE0yteNidbuZDABfjsLOuaq1Q67QUhIvbXjL4vY98+z255+Z9AzVKyh1HUQKv2czh6/h/fL99PhLLmZKWa49fXB2mM1oP1kdEm8BdrAQHsRtRB6DJfIVy3YeaUNMxs/hYV24TV75uxT8EEdC2gn5hjl2EJNW7HuFY7dFXQbS8qbV/0PK7mHA5VndVJnQ/w7NBlGTia1RkgvGIsY01Z5Yv4IBY1gyE8gjCYCwYWkEbeoY2qQsgscikmr73b1gJWbuyr7gcD6KXAfBrIO7GQS7Ra8dq/RwaKHEQy9bdzWEm9/8nd7uUYCmFcI3zNzYnDm05U8Z5RXVE6WcH/sga7dDljPFhGIsRqPkG/V4UfhxYd8n2uwPL+oXomI08mIealuO0bJ2Lgyn1EZLrmYGpaEoulOUlCO6XkngFglAKynqP1LGzi/2S7TjdFfNm9mnPEzH0hKfoSCPV5ifJ77Uw83AwA48xT9SywWlhxcgI0MhL3ndYHItf8uMpRh1F3Zp0FV5+bTORBsa8diyyNvDgOq1d/lknzw8d0bPam8oWFF9lTMG+QGSQ==";

        let enc_key = decrypt_blob_key(username, password, wo_blob_key_hex).unwrap();
        assert_eq!(
            enc_key.to_lower_hex_string(),
            "a8496f85a204e72276265b5620b4f307bc29f5f71c600de4c4a97b373dbc621e"
        );

        let plaintext = decrypt_blob(&enc_key, wo_blob).unwrap();
        assert_eq!(plaintext[..4], [1, 0, 0, 0]);

        let value = parse_value(&plaintext).unwrap();
        let blob = Blob::from_value(&value).unwrap();
        assert_eq!(
            &blob.slip77_key,
            "8280c0855f6e79fcce8712ddee830f04b6f75fc03ffc771a49d71499cce148b6"
        );

        let desc = amp_descriptor(&blob, amp_subaccount, &network, gait_path).unwrap();
        assert_eq!(desc, expected_descriptor);

        // Encrypt blob back
        assert_eq!(value, blob.to_value().unwrap());
        let plaintext_ = from_value(&value).unwrap();
        assert_eq!(value, parse_value(&plaintext_).unwrap());
        let encrypted_blob = encrypt_blob(&enc_key, &plaintext).unwrap();
        assert_eq!(decrypt_blob(&enc_key, &encrypted_blob).unwrap(), plaintext);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    #[ignore] // Requires network connectivity
    async fn test_amp0_ext_mainnet() {
        amp0_addr(Network::Liquid, "userleo456", "userleo456", "").await
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    #[ignore] // Requires network connectivity
    async fn test_amp0_ext_testnet() {
        amp0_addr(
            Network::TestnetLiquid,
            "userleo3456",
            "userleo3456",
            "GA2g7wuT1j4PMPriUGRWhHTcGxMEWV",
        )
        .await
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn amp0_addr(network: Network, username: &str, password: &str, amp_id: &str) {
        let mut amp0ext = Amp0::new_with_network(network, username, password, amp_id)
            .await
            .unwrap();

        if !amp_id.is_empty() {
            assert_eq!(amp0ext.amp_id(), amp_id);
        }

        assert_eq!(amp0ext.amp_subaccount, 1);
        assert!(amp0ext.last_index > 20);
        let desc = amp0ext.wollet_descriptor().to_string();
        println!("{}", desc);

        // Get a new address
        let last_index = amp0ext.last_index;
        let addr = amp0ext.address(None).await.unwrap();
        println!("{:?}", addr);
        assert_eq!(addr.index(), last_index + 1);
        // Last index increased
        assert_eq!(amp0ext.last_index, last_index + 1);

        // Get a previous address
        let addr_prev = amp0ext.address(Some(amp0ext.last_index)).await.unwrap();
        assert_eq!(addr.address(), addr_prev.address());
        // Lasts index did not increased
        assert_eq!(amp0ext.last_index, last_index + 1);

        // Get a future address
        let err = amp0ext
            .address(Some(amp0ext.last_index + 1))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Address index too high"));

        // Address with index 0 is not monitored by Green backend
        // and it must not be used.
        let err = amp0ext.address(Some(0)).await.unwrap_err();
        assert!(err.to_string().contains("Invalid address index for AMP0"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore] // Requires network connectivity
    fn test_amp0_sign_testnet() {
        amp0_sign(false)
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore] // Requires local Green backend
    fn test_amp0_sign_regtest() {
        amp0_sign(true)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn amp0_sign(regtest: bool) {
        use super::*;
        use crate::clients::blocking::BlockchainBackend;
        use crate::{ElectrumClient, ElementsNetwork, Wollet};
        use lwk_common::Network;
        use lwk_common::Signer;
        use lwk_signer::SwSigner;

        let mnemonic = "thrive metal cactus come oval candy medal bounce captain shock permit joke";
        let username = "userlwk001";
        let password = "userlwk001";
        let amp_id = "";

        let (network, elements_network, url) = if regtest {
            (
                Network::LocaltestLiquid,
                ElementsNetwork::default_regtest(),
                "tcp://localhost:19002",
            )
        } else {
            (
                Network::TestnetLiquid,
                ElementsNetwork::LiquidTestnet,
                "ssl://elements-testnet.blockstream.info:50002",
            )
        };

        let electrum_url = url.parse().unwrap();
        let mut client = ElectrumClient::new(&electrum_url).unwrap();

        let mut amp0 = blocking::Amp0::new(network, username, password, amp_id).unwrap();

        if !regtest {
            assert_eq!(amp0.amp_id(), "GA4VUas7QorFN1tamV6vGbhMz8iwkf");
        }

        let wd = amp0.wollet_descriptor();
        let mut wollet = Wollet::without_persist(elements_network, wd).unwrap();

        fn sync(wollet: &mut Wollet, client: &mut impl BlockchainBackend, amp0: &blocking::Amp0) {
            let update = client
                .full_scan_to_index(wollet, amp0.last_index())
                .unwrap();
            if let Some(update) = update {
                wollet.apply_update(update).unwrap();
            }
        }

        sync(&mut wollet, &mut client, &amp0);

        // Check we have enough funds to send a transaction
        let balance = wollet.balance().unwrap();
        println!("Balance: {:?}", balance);
        let lbtc = wollet.policy_asset();
        let balance_before = *balance.get(&lbtc).unwrap_or(&0);
        if balance_before < 500 {
            let addr = amp0.address(Some(1)).unwrap();
            println!("Address: {:?}", addr);
            panic!("Send some tLBTC to {}", addr.address());
        }

        // Construct a PSET sending LBTC back to the wallet
        let amp0pset = wollet
            .tx_builder()
            .drain_lbtc_wallet()
            .finish_for_amp0()
            .unwrap();
        let mut pset = amp0pset.pset().clone();
        let blinding_nonces = amp0pset.blinding_nonces();

        // User signs the PSET
        let signer = SwSigner::new(mnemonic, false).unwrap();
        let sigs = signer.sign(&mut pset).unwrap();
        assert!(sigs > 0);

        // Reconstruct the Amp0 PSET with the PSET signed by the user
        let amp0pset = Amp0Pset::new(pset, blinding_nonces.to_vec()).unwrap();

        // AMP0 signs
        let tx = amp0.sign(&amp0pset).unwrap();

        // Broadcast the transaction
        let txid = client.broadcast(&tx).unwrap();
        println!("txid: {}", txid);

        // Apply the transaction
        wollet.apply_transaction(tx).unwrap();

        let balance_after = *wollet.balance().unwrap().get(&lbtc).unwrap();
        assert!(balance_after < balance_before);
    }

    #[test]
    fn amp0_fingerprint() {
        let xpub = server_master_xpub(&Network::Liquid);
        assert_eq!(xpub.fingerprint().to_string(), AMP0_FINGERPRINT_MAINNET);

        let xpub = server_master_xpub(&Network::TestnetLiquid);
        assert_eq!(xpub.fingerprint().to_string(), AMP0_FINGERPRINT_TESTNET);

        let xpub = server_master_xpub(&Network::LocaltestLiquid);
        assert_eq!(xpub.fingerprint().to_string(), AMP0_FINGERPRINT_REGTEST);
    }

    #[test]
    fn test_gait_path() {
        use elements::bitcoin::bip32::{DerivationPath, Xpriv};
        use elements::bitcoin::Network;

        let mnemonic = "student lady today genius gentle zero satoshi book just link gauge tooth";
        // From GDK logs
        let gait_path_hex = "5856d4bbb94724768c337e1cc547b48df2b68068b9399f1c2d84f1a6c5824eb4788d3c17b2635cf8f90de4c2d3a2ba3284f6518d843f6801ac9894c033e4640f";

        let mnemonic: bip39::Mnemonic = mnemonic.parse().unwrap();
        let seed = mnemonic.to_seed("");
        let master_xprv = Xpriv::new_master(Network::Testnet, &seed).unwrap();

        // 18241 = 0x4741 = 'GA'
        let register_path: DerivationPath = "m/18241h".parse().unwrap();
        let xprv = master_xprv.derive_priv(&EC, &register_path).unwrap();
        let xpub = Xpub::from_priv(&EC, &xprv);

        assert_eq!(derive_gait_path(&xpub), gait_path_hex)
    }

    #[test]
    fn test_slow_amp0_hmac() {
        use elements::bitcoin::bip32::{DerivationPath, Xpriv};
        use elements::bitcoin::Network;

        let mnemonic = "student lady today genius gentle zero satoshi book just link gauge tooth";
        // From GDK logs
        let blob64 = "4O9yRhKbJQubAgJKrRW60pBc9JK5RhUb+GA8eDbG2H2ajnCYg4G2YMKBcCrAuemyNox0RYLS4qjeQG93wUlkpSenjeTdpyUXP68iavsQQi0744DYY7Owce5qaKZx2Uv1Z0a7Ta+DtEaVBpYi7a8MjOdw3u5pnFFq9H0pWweuWc2pz7Vj8GoruCzitSQaWdJ81P1nZZjaSpYclDlVU/nlvee4LXhMmNIAhNhFiZOOt0d/G3F/v1xdirWRwoZ38b5cP+ieeiqvwJ0GccGDr4qPqgC4w7pc6IK+PVUUmh9nyu5iVr/VRyn+uwv2QUl3jyPObqJ67qwV0LL2hL1aAkAraah1WXb2CZP4o947zAxb5hTkqPjqrXFEHjxW9IBkOSSo/1UKF4wnWtvrSvePeSZmWQffKQIfBXMB3RQE+E53bW2c1waD6DCwurdPQuiZJNe2WDKXsBRdwn548VLD91AyJYTLmP87H4X4TXDSo6HXLJfZf7r8qFMJhy4yFYgTWtrPun+9NsCZ2p1/AUAmihZWchsyC/O6hMP4iowJsW0TGZCeSWZTHSa8iKbnDj29vWKLd5DnQ0ePZTmi8wuJSKZy020mFp2czvT6qpBu3txLDuwrltLNMxSlcMaNi0rvICArM+E8v0lmdPlKLdkkvwAHjp8G5Dj+rv9qNvI84S2W/GBgugM0aLXefsn+PH+hxoi4m296ToHJiZhzr774pqgvEeiaUs4TXhVukJiupRUa3/EB37QyikreNZaLIw==";
        let hmac64 = "OM/H4321wV0n+MQXln4UnL2uBBLgB7EScJ+ZuZEDNQU=";

        let mnemonic: bip39::Mnemonic = mnemonic.parse().unwrap();
        let seed = mnemonic.to_seed("");
        let master_xprv = Xpriv::new_master(Network::Testnet, &seed).unwrap();

        // 1885434739 = 0x70617373 = 'pass'
        let client_secret_path: DerivationPath = "m/1885434739h".parse().unwrap();
        let xprv = master_xprv.derive_priv(&EC, &client_secret_path).unwrap();
        let client_secret_xpub = Xpub::from_priv(&EC, &xprv);

        let (enc_key, hmac_key) = derive_blob_keys(&client_secret_xpub);
        assert_eq!(compute_hmac(&hmac_key, blob64).unwrap(), hmac64);

        let username = "userleo345678";
        let password = "userleo345678";
        let wo_blob_key_hex = encrypt_blob_key(username, password, &enc_key).unwrap();
        assert_eq!(
            decrypt_blob_key(username, password, &wo_blob_key_hex).unwrap(),
            enc_key
        );
    }

    #[test]
    fn test_amp0_signer() {
        use lwk_common::Amp0Signer;
        use lwk_signer::SwSigner;

        let mnemonic = "student lady today genius gentle zero satoshi book just link gauge tooth";
        // From GDK logs
        let master_public_key =
            "03f07921310eea86e351e5c6d9d8d65284b1cdbb67125c2baf6aef5ff65885582e";
        let master_chain_code = "ff2842bb066b088825cbd4ad8267ba86e7d989ebf333465d0106ac632b317096";
        let gait_path_hex = "5856d4bbb94724768c337e1cc547b48df2b68068b9399f1c2d84f1a6c5824eb4788d3c17b2635cf8f90de4c2d3a2ba3284f6518d843f6801ac9894c033e4640f";
        let login_address = "2dofAJ9jV6MjS2NLMp17nVYVZL4Z5s8Sm47";

        let challenge = "BZVE6";
        let der_sig_hex = "30440220717c2a05640bb52eecb577fcf725c2e93e1efabd3a2a56450308340f0b17a6c2022051cf947d49315866540e6c2d2dcb3f2d25d4f3593cd9a2b53d449a9480d3e379";

        let subaccount_num = 1;
        let account_xpub = "tpubDA9GDAo3JyS2TaEikypKnu21N8sjLfTawM5te2jy9poCbFvYmRwSCz7Hk3YQiuMyStm1suBGTEW21ztSkisovDnyqo5nK1CgSY3LJesEci7";

        let signer = SwSigner::new(mnemonic, false).unwrap();
        let signer_data = signer.amp0_signer_data().unwrap();
        let master_xpub = signer_data.master_xpub();
        assert_eq!(master_public_key, master_xpub.public_key.to_hex());
        assert_eq!(master_chain_code, master_xpub.chain_code.to_hex());
        let register_xpub = signer_data.register_xpub();

        assert_eq!(derive_gait_path(register_xpub), gait_path_hex);

        let network = lwk_common::Network::LocaltestLiquid;
        assert_eq!(
            signer_data.login_address(&network).to_string(),
            login_address
        );

        assert_eq!(signer.amp0_sign_challenge(challenge).unwrap(), der_sig_hex);

        let xpub = signer.amp0_account_xpub(subaccount_num).unwrap();
        let account_xpub: Xpub = account_xpub.parse().unwrap();
        assert_eq!(xpub.public_key, account_xpub.public_key);
        assert_eq!(xpub.chain_code, account_xpub.chain_code);

        assert_eq!(xpub.network, account_xpub.network);
        assert_eq!(xpub.depth, account_xpub.depth);
        assert_eq!(xpub.child_number, account_xpub.child_number);
        // parent_fingerprint does not match because it skips hash computation
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "Requires network connectivity"]
    fn test_amp0_full_login_testnet() {
        amp0_full_login(false)
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "Requires local Green backend"]
    fn test_amp0_full_login_regtest() {
        amp0_full_login(true)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn amp0_full_login(regtest: bool) {
        use super::*;
        use lwk_common::Amp0Signer;
        use lwk_common::Network;
        use lwk_signer::SwSigner;

        let network = if regtest {
            Network::LocaltestLiquid
        } else {
            Network::TestnetLiquid
        };

        // Create signer and watch only credentials
        let (signer, mnemonic) = SwSigner::random(false).unwrap();
        let username = format!("user{}", signer.fingerprint());
        let password = format!("pass{}", signer.fingerprint());
        println!("mnemonic: {}", mnemonic);
        println!("username: {}", username);
        println!("password: {}", password);

        // Collect signer data
        let signer_data = signer.amp0_signer_data().unwrap();
        // Connect to AMP0
        let amp0 = blocking::Amp0Connected::new(network, signer_data).unwrap();
        // Obtain and sign the authentication challenge
        let challenge = amp0.get_challenge().unwrap();
        let sig = signer.amp0_sign_challenge(&challenge).unwrap();
        // Login
        let mut amp0 = amp0.login(&sig).unwrap();
        // Create a new AMP0 account
        let pointer = amp0.next_account().unwrap();
        let account_xpub = signer.amp0_account_xpub(pointer).unwrap();
        let amp_id = amp0.create_amp0_account(pointer, &account_xpub).unwrap();
        assert_eq!(&amp_id[..2], "GA");
        assert!(amp0.get_amp_ids().unwrap().contains(&amp_id));
        // Create watch only entries
        amp0.create_watch_only(&username, &password).unwrap();

        // Use watch only credentials to interact with AMP0
        let _amp0 = blocking::Amp0::new(network, &username, &password, &amp_id).unwrap();
    }

    #[test]
    fn test_amp0_blobs_from_gdk() {
        // Check that we can correctly read blobs created by GDK
        use lwk_common::Amp0Signer;
        use lwk_signer::SwSigner;

        // Create signer and derive the blob keys
        let mnemonic = "deny forum retreat basic step cook boring say october owner fun trade";
        let signer = SwSigner::new(mnemonic, false).unwrap();
        let signer_data = signer.amp0_signer_data().unwrap();
        let (enc_key, hmac_key) = derive_blob_keys(signer_data.client_secret_xpub());

        // No watch only, no AMP0 account
        let blob64 = "VDdsBF30NkJiA77X9Ynv87U/NZsqSu+ENXCDljDc+SgIppjAjySP7+CZQiZXDBaOMjtCVuqRLUjA8zpOQ9CjWlfiftLzrSe9k+sZXVK0/9hl0oLpz5Euh7CL7T+Hqlk/TqXNAZBgwmnpx8y0XThJqNFo07IAgR9ByAoqSY/t/cC5ltVSdWUqsY5u1ZQM7y+UN3YPDudWFAhfGe3GiQA8+PYCBMo9+OPNwKCZwr3Vt2iuPj/p3KUxG6xpFb08byf7yONCYrRQA65Gyvn4hVnujDu9X8rTM3pZA138ecboEbW1mmUFz4TZs9xyKRWPoxOq/eeBh55j5twvN9Uuo8nOwYUhBg0kpt29dU9X8H3zy4rIMVbDSjmcyeFmK+Zx4qDbScZ/9hwHv2aBZVbg3wnMgTdwg0Ss+zp1yAOSqxik56sd25EZHCE4i/2zxysCWjxpsZMtswWeRCba27OQem2RoPs7rWIyyFQ5GDnyI6TxxfCsiZBvb8UlLA/DRJJGPgN+IvVOk8HIfB4HOkKvPc//8GBva7fciS+IJF93frQOVUyLlSVHX9ungz5O4+07LpfiX48cVYrRiLLQ8P5Iura6i2/cAw2IBo94FLEt8jbCSATCoV+HU8dO";
        let hmac = "gfXz+WTkXVWWhtDLkI6lnf9Zq2hqs9Iu7+YMySaPkY0=";
        assert_eq!(compute_hmac(&hmac_key, blob64).unwrap(), hmac);
        let blob = Blob::from_base64(blob64, &enc_key).unwrap();

        assert_eq!(blob.xpubs.len(), 3); // master, login, client secret
        fn erase_parent_fp(xpub: &Xpub) -> Xpub {
            let mut xpub = *xpub;
            xpub.parent_fingerprint = Default::default();
            xpub
        }
        let master_xpub = erase_parent_fp(signer_data.master_xpub());
        let login_xpub = erase_parent_fp(signer_data.login_xpub());
        let client_secret_xpub = erase_parent_fp(signer_data.client_secret_xpub());
        assert!(blob.xpubs.contains_key(&master_xpub));
        assert!(blob.xpubs.contains_key(&login_xpub));
        assert!(blob.xpubs.contains_key(&client_secret_xpub));

        assert_eq!(blob.watchonly.len(), 1);
        assert!(blob.watchonly.contains_key("xpubs"));
        // No username key

        // Create watch only
        let blob64 = "xDNNtfairFXjFrV38FFCAtztjoJB0g1upBcra6Ug5Pg2q++/7iIhCXcBeZKAOoL3ao64g3vCF2uEsE8lJTPllyP5yU6wKHiskCt4ds7C889kMBG7XRY3q8feed8c54sYxxCZFLdLgi4O3YyETdpPtVn8nbPEGi2gky+PIZTlwl0ngw4obr+joOr3gfcnefDIMm6/2crTXdDQjxMccB+eWLPUpaGEoUcf4hvy8ZeZ5FIgUcn+iRaOqNpq9BMXo9o0d2WhqFILqe5QIk/wsVT7sT/OY3EQ3FB4tHSewuuwQb7I4V6Ri7Ykf1vzvg4GYvjbaUhOSBD3MIn1QigEtt2RjevI9lLox1qzALGJWViz6HDZ5xy5G8mpMpBKka2pb9H+eX5Po13vyOsqQGrvd53AVYkQIMWUSKLRj/KlI+6dKmY3LQWevxPKMoF8VUmGgjwmHOciHzKTcGKwWFcwI7P+1SkUlGzsrINYvSFD3D9HRVwebYICT0vf8rQMbYeDMe1D25Yy9QWcNIL+fa6As/zQblncoiZUqlxUNuv0CqAAvpQcxBkqIbD0Mscrcp4ZIhcFpcrrqAohLswxi3c8BAbuxpudV14zbcNp63GBzuA2JauYfxjPnxAizhMvRZ9RS5Be7zAPYP1heV73XQ==";
        let hmac = "H3iXm81gKH579Jv16ilQxxR6HYD04IwiQ38xdK9KtmA=";
        assert_eq!(compute_hmac(&hmac_key, blob64).unwrap(), hmac);
        let blob = Blob::from_base64(blob64, &enc_key).unwrap();

        assert_eq!(blob.xpubs.len(), 3); // master, login, client secret

        assert_eq!(blob.watchonly.len(), 2);
        assert!(blob.watchonly.contains_key("xpubs"));
        assert_eq!(
            blob.watchonly.get("username").unwrap().to_string(),
            "\"testenckey2\""
        );

        // Create subaccount
        let blob64 = "hwZ89hWUsWCWMInEOkxBztqG+K9dsE0tL+H4fwzxIyUyDf3J545Ski9MYjLS6uuAYsuhXdRUsoVtJZhJOBTziVoTYXHs+M4V+KZWAfJeHZflL5ViWFvbnEGil8occK8QeEzzjg4dD8uzSb+DLhDyVE9CTXxG8eaMklKgQ5+tZ6XWR4WBKlA4O6gf4VaIIjkaRKNgOB6PaJC2X5NZNBlSPM+2gNfaL3E9J6pwXlqrBOzN3F7SCny3+LWmUdJTwPrZqKDTPGjXTZgFyrofkBeNyFtyoziSI9TsmXppBaLWpRy4EZPHHOZmFeXhPmjmIiasq4p1lIfVkMyk3HWb3tV7lcg0O57Ghp173gWbRqY0iDn3w8F8Z31jMpKbADP2DPRVQHPst4cqEjmCAmzi3AipeX913ZTtHOIXxWAuzQvpERyfV7GjQfMpjc985JMwNJijMedq0cYPuOPUKkmoSgTZVzNL2/Gh5csfZ+8K5u5wwGF09ZSAKhty0anWwv7S6ALe4HuuMa6dU2LXpYClrtel7QY1aPbZBuIZdIW6tWvU7uJR5j6NsA5V+3N/gn4YVzpZEX5FOd0AowFqi1XaCJ7UhPBqgXr8ODCi7PdXaMVRvt7pb4o2pVM+8cdDLePYQEesIB6m8P93fDen0zk9W01xNtJeDX7KJTT+DDu7zN2hg+yzXEF6g1aDQzP5oPrRssXrf4g/IPDxCVepQ+kEFC5wpNEREQNPqnCVoTdVdSKKimexjVyxl6ygKjtSnJIvhOq6MC99/MO0m6oILA4uxXwCFA==";
        let hmac = "L+stojS55bSdNL7ke9KQkvyI7m3AAGLUwmCaPeYtf8A=";
        assert_eq!(compute_hmac(&hmac_key, blob64).unwrap(), hmac);
        let blob = Blob::from_base64(blob64, &enc_key).unwrap();

        assert_eq!(blob.xpubs.len(), 4);
        let account_xpub = erase_parent_fp(&signer.amp0_account_xpub(1).unwrap());
        assert!(blob.xpubs.contains_key(&account_xpub));

        assert_eq!(blob.watchonly.len(), 2);
    }

    #[test]
    fn test_amp0_signer_data_serde() {
        use lwk_common::Amp0Signer;
        use lwk_signer::SwSigner;

        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let signer = SwSigner::new(mnemonic, false).unwrap();

        let sd = signer.amp0_signer_data().unwrap();
        let sd_str = sd.to_string();
        assert_eq!(Amp0SignerData::from_str(&sd_str).unwrap(), sd);
    }
}
