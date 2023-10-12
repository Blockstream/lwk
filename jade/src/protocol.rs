use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

use crate::{
    error::ErrorDetails,
    get_receive_address::GetReceiveAddressParams,
    register_multisig::RegisterMultisigParams,
    sign_liquid_tx::{SignLiquidTxParams, TxInputParams},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Request<P> {
    pub id: String,
    pub method: String,
    pub params: Option<P>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Params {
    Epoch(EpochParams),
    Entropy(EntropyParams),
    AuthUser(AuthUserParams),
    Handshake(HandshakeParams),
    UpdatePinServer(UpdatePinserverParams),
    HandshakeComplete(HandshakeCompleteParams),
    GetXpub(GetXpubParams),
    GetReceiveAddress(GetReceiveAddressParams),
    SignMessage(SignMessageParams),
    GetSignature(GetSignatureParams),
    SignLiquidTx(SignLiquidTxParams),
    DebugSetMnemonic(DebugSetMnemonicParams),
    TxInput(TxInputParams),
    RegisterMultisig(RegisterMultisigParams),
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
pub struct HandshakeParams {
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

#[derive(Debug, Deserialize, Serialize)]
pub struct SignMessageParams {
    pub message: String,
    pub path: Vec<u32>,

    #[serde(with = "serde_bytes")]
    pub ae_host_commitment: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetSignatureParams {
    /// 32 bytes anti-exfiltration entropy
    #[serde(with = "serde_bytes")]
    pub ae_host_entropy: Vec<u8>,
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

#[derive(Debug, Deserialize, Serialize)]
pub struct PingResult(u8);

#[derive(Debug, Deserialize, Serialize)]
pub struct BoolResult(bool);

impl BoolResult {
    pub fn get(&self) -> bool {
        self.0
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ByteResult(ByteBuf);

impl From<ByteResult> for Vec<u8> {
    fn from(value: ByteResult) -> Self {
        value.0.into_vec()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct VersionInfoResult {
    jade_version: String,
    jade_ota_max_chunk: u32,
    jade_config: String,
    board_type: String,
    jade_features: String,
    idf_version: String,
    chip_features: String,
    efusemac: String,
    battery_status: u8,
    jade_state: String,
    jade_networks: String,
    pub jade_has_pin: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisteredMultisig {
    variant: String,
    sorted: bool,
    threshold: u32,
    num_signers: u32,

    #[serde(with = "serde_bytes")]
    master_blinding_key: Vec<u8>,
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

#[derive(Debug, Deserialize, Serialize)]
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
