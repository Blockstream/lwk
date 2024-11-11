use std::fmt::Debug;

use elements::hex::ToHex;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_cbor::Value;

use crate::{
    error::ErrorDetails,
    get_receive_address::GetReceiveAddressParams,
    register_multisig::{GetRegisteredMultisigParams, RegisterMultisigParams},
    sign_liquid_tx::{SignLiquidTxParams, TxInputParams},
    Network,
};

#[derive(Debug, Serialize)]
pub struct FullRequest {
    pub id: String,
    pub method: String,
    pub params: Request,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Request {
    Ping,
    Logout,
    GetVersionInfo,
    SetEpoch(EpochParams),
    AddEntropy(EntropyParams),
    AuthUser(AuthUserParams),
    UpdatePinserver(UpdatePinserverParams),
    GetXpub(GetXpubParams),
    GetReceiveAddress(GetReceiveAddressParams),
    GetMasterBlindingKey(GetMasterBlindingKeyParams),
    SignMessage(SignMessageParams),
    GetSignature(GetSignatureParams),
    SignLiquidTx(SignLiquidTxParams),
    TxInput(TxInputParams),
    DebugSetMnemonic(DebugSetMnemonicParams),
    RegisterMultisig(RegisterMultisigParams),
    GetRegisteredMultisigs,
    GetRegisteredMultisig(GetRegisteredMultisigParams),
    Generic(GenericMethod),
}

#[derive(Debug, Serialize)]
pub struct GenericMethod {
    #[serde(skip)]
    pub(crate) method: String,
    #[serde(flatten)]
    pub(crate) params: serde_cbor::Value,
}

impl std::fmt::Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Request::Ping => write!(f, "ping"),
            Request::Logout => write!(f, "logout"),
            Request::GetVersionInfo => write!(f, "get_version_info"),
            Request::SetEpoch(_) => write!(f, "set_epoch"),
            Request::AddEntropy(_) => write!(f, "add_entropy"),
            Request::AuthUser(_) => write!(f, "auth_user"),
            Request::UpdatePinserver(_) => write!(f, "update_pinserver"),
            Request::GetXpub(_) => write!(f, "get_xpub"),
            Request::GetReceiveAddress(_) => write!(f, "get_receive_address"),
            Request::GetMasterBlindingKey(_) => write!(f, "get_master_blinding_key"),
            Request::SignMessage(_) => write!(f, "sign_message"),
            Request::GetSignature(_) => write!(f, "get_signature"),
            Request::SignLiquidTx(_) => write!(f, "sign_liquid_tx"),
            Request::TxInput(_) => write!(f, "tx_input"),
            Request::DebugSetMnemonic(_) => write!(f, "debug_set_mnemonic"),
            Request::RegisterMultisig(_) => write!(f, "register_multisig"),
            Request::GetRegisteredMultisigs => write!(f, "get_registered_multisigs"),
            Request::GetRegisteredMultisig(_) => write!(f, "get_registered_multisig"),
            Request::Generic(g) => write!(f, "{0}", g.method),
        }
    }
}

impl Request {
    pub fn network(&self) -> Option<Network> {
        match self {
            Request::GetXpub(e) => Some(e.network),
            Request::GetReceiveAddress(e) => Some(e.network),
            Request::SignLiquidTx(e) => Some(e.network),
            Request::RegisterMultisig(e) => Some(e.network),
            _ => None,
        }
    }
}

impl Request {
    pub fn serialize(self) -> Result<Vec<u8>, crate::Error> {
        let mut rng = rand::thread_rng();
        let id = rng.next_u32().to_string();
        let method = self.to_string();
        let req = FullRequest {
            id,
            method,
            params: self,
        };
        let mut buf = Vec::new();
        serde_cbor::to_writer(&mut buf, &req)?;
        log::debug!(
            "\n--->\t{:#?}\n\t({} bytes) {}",
            &req,
            buf.len(),
            &hex::encode(&buf),
        );
        Ok(buf)
    }
}

#[derive(Debug, Serialize)]
pub struct AuthUserParams {
    pub network: crate::Network,
    pub epoch: u64,
}

impl AuthUserParams {
    pub fn new(network: crate::Network) -> Self {
        let epoch = web_time::SystemTime::now()
            .duration_since(web_time::SystemTime::UNIX_EPOCH)
            .map(|e| e.as_secs())
            .unwrap_or(0);
        AuthUserParams { network, epoch }
    }
}

#[derive(Debug, Serialize)]
pub struct EpochParams {
    pub epoch: u64,
}

#[derive(Debug, Serialize)]
pub struct EntropyParams {
    #[serde(with = "serde_bytes")]
    pub entropy: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetXpubParams {
    pub network: crate::Network,

    /// Derive the master node (m) with the given path and the return the resuting xpub
    pub path: Vec<u32>,
}

#[derive(Deserialize, Serialize)]
pub struct SignMessageParams {
    pub message: String,
    pub path: Vec<u32>,

    #[serde(with = "serde_bytes")]
    pub ae_host_commitment: Vec<u8>,
}

impl Debug for SignMessageParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignMessageParams")
            .field("message", &self.message)
            .field("path", &self.path)
            .field("ae_host_commitment", &self.ae_host_commitment.to_hex())
            .finish()
    }
}

#[derive(Deserialize, Serialize)]
pub struct GetSignatureParams {
    /// 32 bytes anti-exfiltration entropy
    #[serde(with = "serde_bytes")]
    pub ae_host_entropy: Vec<u8>,
}

impl Debug for GetSignatureParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GetSignatureParams")
            .field("ae_host_entropy", &self.ae_host_entropy.to_hex())
            .finish()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HandshakeComplete {
    pub encrypted_data: String,
    pub hmac_encrypted_data: String,
    pub ske: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DebugSetMnemonicParams {
    pub mnemonic: String,
    pub passphrase: Option<String>,
    pub temporary_wallet: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Response<T> {
    pub id: String,
    pub result: Option<T>,
    pub error: Option<ErrorDetails>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct VersionInfoResult {
    pub jade_version: String,
    pub jade_ota_max_chunk: u32,
    pub jade_config: String,
    pub board_type: String,
    pub jade_features: String,
    pub idf_version: String,
    pub chip_features: String,
    pub efusemac: String,
    pub battery_status: u8,
    pub jade_state: JadeState,
    pub jade_networks: String,
    pub jade_has_pin: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]

pub enum JadeState {
    /// no wallet set on the hw, mnemonic not entered, unit uninitialised
    Uninit,

    /// wallet mnemonic has been set on hw, but not yet persisted with blind pinserver
    Unsaved,

    /// wallet set, but currently locked - requires PIN entry to unlock.
    Locked,

    /// wallet set and unlocked for this interface, ready to use.
    Ready,

    ///  hw currently set with a temporary ('Emergency Restore') wallet, ready to use.
    Temp,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum IsAuthResult {
    AlreadyAuth(bool),
    AuthResult(AuthResult),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthResult {
    http_request: HttpRequest,
}

impl AuthResult {
    /// Given a set of urls select the most appropriate
    /// Preference is in order:
    ///  - onion
    ///  - https
    ///  - http
    ///
    /// onion urls are ignored if use_tor is false.
    /// TODO: at the moment LWK doesn't support TOR
    pub fn url(&self, use_tor: bool) -> Option<&str> {
        select_url(&self.http_request.params.urls, use_tor)
    }

    pub fn data(&self) -> &Value {
        &self.http_request.params.data
    }

    pub fn on_reply(&self) -> &str {
        &self.http_request.on_reply
    }
}

fn select_url(urls: &[String], use_tor: bool) -> Option<&str> {
    let (onion, clear): (Vec<_>, Vec<_>) = urls.iter().partition(|e| e.ends_with(".onion"));
    if use_tor {
        onion.first()
    } else {
        let https = clear.iter().find(|e| e.starts_with("https://"));
        match https {
            Some(url) => Some(url),
            None => clear.first(),
        }
    }
    .map(|e| e.as_str())
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HttpRequest {
    params: HttpParams,
    #[serde(rename = "on-reply")]
    on_reply: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct HttpParams {
    urls: Vec<String>,
    method: String,
    accept: String,

    /// Generic data, must remain opaque for protocol upgrades
    data: Value,
}

#[derive(Deserialize, Serialize)]
pub struct UpdatePinserverParams {
    pub reset_details: bool,
    pub reset_certificate: bool,

    #[serde(rename = "urlA")]
    pub url_a: String,

    #[serde(rename = "urlB")]
    pub url_b: String,

    #[serde(with = "serde_bytes")]
    pub pubkey: Vec<u8>,
    pub certificate: String,
}

impl Debug for UpdatePinserverParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdatePinserverParams")
            .field("reset_details", &self.reset_details)
            .field("reset_certificate", &self.reset_certificate)
            .field("url_a", &self.url_a)
            .field("url_b", &self.url_b)
            .field("pubkey", &self.pubkey.to_hex())
            .field("certificate", &self.certificate)
            .finish()
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct GetMasterBlindingKeyParams {
    pub only_if_silent: bool,
}

#[cfg(test)]
mod test {
    use crate::protocol::select_url;

    #[test]
    fn serialize_empty() {
        let a = super::Request::Ping;
        let s = serde_json::to_string(&a).unwrap();
        assert_eq!(s, "null");
    }

    #[test]
    fn test_select_url() {
        let http = "http://ciao.it";
        let https = "https://ciao.it";
        let onion = "http://mrrxtq6tjpbnbm7vh5jt6mpjctn7ggyfy5wegvbeff3x7jrznqawlmid.onion";

        let urls = [http.to_owned(), https.to_owned()];
        assert_eq!(select_url(&urls, false), Some(https));

        let urls = [onion.to_owned()];
        assert_eq!(select_url(&urls, false), None);

        let urls = [onion.to_owned(), https.to_owned()];
        assert_eq!(select_url(&urls, false), Some(https));

        let urls = [onion.to_owned(), http.to_owned()];
        assert_eq!(select_url(&urls, false), Some(http));

        let urls = [https.to_owned(), onion.to_owned()];
        assert_eq!(select_url(&urls, true), Some(onion));
    }
}
