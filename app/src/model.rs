use serde::{Deserialize, Serialize};
use signer::{Signer, SignerError};
use std::collections::HashMap;
use wollet::bitcoin::bip32::ExtendedPubKey;
use wollet::bitcoin::hash_types::XpubIdentifier;
use wollet::elements::{Address, AssetId};
use wollet::UnvalidatedAddressee;

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
    pub signers: Vec<SignerResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadWalletRequest {
    pub descriptor: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletResponse {
    pub descriptor: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListWalletsResponse {
    pub wallets: Vec<WalletResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadWalletRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadWalletResponse {
    pub unloaded: WalletResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SignerKind {
    Software,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSignerRequest {
    pub name: String,
    pub kind: String,
    pub mnemonic: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadSignerRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadSignerResponse {
    pub unloaded: SignerResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignerResponse {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SendRequest {
    pub addressees: Vec<UnvalidatedAddressee>,
    pub fee_rate: Option<f32>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendResponse {
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SinglesigDescriptorResponse {
    pub descriptor: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SinglesigDescriptorRequest {
    pub name: String,
    pub descriptor_blinding_key: String,
    pub singlesig_kind: String,
}

impl<'a> TryFrom<(String, &Signer<'a>)> for SignerResponse {
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
