use serde::{Deserialize, Serialize};
use signer::{Signer, SignerError};
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
pub struct ListSignersResponse {
    pub signers: Vec<LoadSignerResponse>,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListWalletsResponse {
    pub wallets: Vec<LoadWalletResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadWalletRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadWalletResponse {
    pub name: String,
    pub descriptor: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSignerRequest {
    pub mnemonic: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadSignerRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadSignerResponse {
    pub name: String,
    pub identifier: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSignerResponse {
    pub name: String,
    pub id: XpubIdentifier,
    pub fingerprint: String,
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

impl<'a> TryFrom<(String, &Signer<'a>)> for LoadSignerResponse {
    type Error = SignerError;

    fn try_from(name_and_signer: (String, &Signer<'a>)) -> Result<Self, Self::Error> {
        let (name, signer) = name_and_signer;
        let fingerprint = signer.fingerprint()?.to_string();
        let xpub = signer.xpub()?;
        let id = signer.id()?;

        Ok(Self {
            name,
            id,
            fingerprint,
            xpub,
        })
    }
}
