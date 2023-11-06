use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wollet::bitcoin::bip32::ExtendedPubKey;
use wollet::bitcoin::hash_types::XpubIdentifier;
use wollet::elements::{Address, AssetId};

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
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadWalletResponse {
    pub descriptor: String,
    pub name: String,
    pub new: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSignerRequest {
    pub mnemonic: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSignerResponse {
    pub name: String,
    pub id: XpubIdentifier,
    pub fingerprint: String,
    pub new: bool,
    pub xpub: ExtendedPubKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddressRequest {
    pub name: String,
    pub index: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddressResponse {
    pub address: Address,
    pub index: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub balance: HashMap<AssetId, u64>,
}
