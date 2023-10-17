use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct SignerGenerateResponse {
    pub mnemonic: String,
}
