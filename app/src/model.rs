use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct SignerGenerateResponse {
    pub mnemonic: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoadWalletRequest {
    pub descriptor: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoadWalletResponse {
    pub descriptor: String,
    pub new: bool,
}

#[derive(Serialize, Deserialize)]
pub struct LoadSignerRequest {
    pub mnemonic: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoadSignerResponse {
    pub fingerprint: String,
    pub new: bool,
}
