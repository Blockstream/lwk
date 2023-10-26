use serde::{Deserialize, Serialize};
use wollet::bitcoin::bip32::ExtendedPubKey;

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateSignerResponse {
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadWalletRequest {
    pub descriptor: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadWalletResponse {
    pub descriptor: String,
    pub new: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSignerRequest {
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSignerResponse {
    pub fingerprint: String,
    pub new: bool,
    pub xpub: ExtendedPubKey,
}
