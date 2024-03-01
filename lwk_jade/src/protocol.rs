use std::fmt::Debug;

use elements::hex::ToHex;
use serde::{Deserialize, Serialize};

use crate::{
    error::ErrorDetails,
    get_receive_address::GetReceiveAddressParams,
    register_multisig::{GetRegisteredMultisigParams, RegisterMultisigParams},
    sign_liquid_tx::{SignLiquidTxParams, TxInputParams},
};

#[derive(Debug, Serialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    pub params: Option<Params>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Params {
    Ping,
    Logout,
    GetVersionInfo,
    SetEpoch(EpochParams),
    AddEntropy(EntropyParams),
    AuthUser(AuthUserParams),
    HandshakeInit(HandshakeInitParams),
    UpdatePinserver(UpdatePinserverParams),
    HandshakeComplete(HandshakeCompleteParams),
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
}

#[derive(Debug, Serialize)]
pub struct AuthUserParams {
    pub network: crate::Network,
    pub epoch: u64,
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
pub struct HandshakeInitParams {
    pub sig: String,
    pub ske: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HandshakeCompleteParams {
    pub encrypted_key: String,
    pub hmac: String,
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
pub enum IsAuthResult<T> {
    AlreadyAuth(bool),
    AuthResult(AuthResult<T>),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthResult<T> {
    http_request: HttpRequest<T>,
}

impl<T> AuthResult<T> {
    pub fn urls(&self) -> &[String] {
        self.http_request.params.urls.as_slice()
    }

    pub fn data(&self) -> &T {
        &self.http_request.params.data
    }
}
#[derive(Debug, Deserialize, Serialize)]
pub struct HttpRequest<T> {
    params: HttpParams<T>,
    #[serde(rename = "on-reply")]
    on_reply: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct HttpParams<T> {
    urls: Vec<String>,
    method: String,
    accept: String,
    data: T,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HandshakeData {
    cke: String,
    encrypted_data: String,
    hmac_encrypted_data: String,
    ske: String,
    error: Option<String>,
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
    #[test]
    fn serialize_empty() {
        let a = super::Params::Ping;
        let s = serde_json::to_string(&a).unwrap();
        assert_eq!(s, "null");
    }
}
