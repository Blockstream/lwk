use common::Signer;
use serde::{Deserialize, Serialize};
use signer::{AnySigner, SignerError};
use std::collections::HashMap;
use wollet::bitcoin::bip32::{ExtendedPubKey, Fingerprint};
use wollet::bitcoin::hash_types::XpubIdentifier;
use wollet::elements::{Address, AssetId, Txid};
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
pub struct PsetResponse {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct MultisigDescriptorRequest {
    pub descriptor_blinding_key: String,
    pub multisig_kind: String,
    pub threshold: u32,
    pub keyorigin_xpubs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultisigDescriptorResponse {
    pub descriptor: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct XpubRequest {
    pub name: String,
    pub xpub_kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct XpubResponse {
    pub keyorigin_xpub: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignRequest {
    pub name: String,
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BroadcastRequest {
    pub name: String,
    pub dry_run: bool,
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BroadcastResponse {
    pub txid: Txid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletDetailsRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IssueRequest {
    pub name: String,
    pub satoshi_asset: u64,
    pub address_asset: Option<String>,
    pub satoshi_token: u64,
    pub address_token: Option<String>,
    pub contract: Option<String>,
    pub fee_rate: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WalletType {
    Unknown,
    Wpkh,
    ShWpkh,
    WshMulti(usize, usize),
}

impl std::fmt::Display for WalletType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            WalletType::Unknown => write!(f, "unknown"),
            WalletType::Wpkh => write!(f, "wpkh"),
            WalletType::ShWpkh => write!(f, "sh_wpkh"),
            WalletType::WshMulti(threshold, num_pubkeys) => {
                write!(f, "wsh_multi_{}of{}", threshold, num_pubkeys)
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignerDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub fingerprint: Fingerprint,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletDetailsResponse {
    #[serde(rename = "type")]
    pub type_: String,
    pub signers: Vec<SignerDetails>,
    pub warnings: String,
}

impl TryFrom<(String, &AnySigner)> for SignerResponse {
    type Error = SignerError;

    fn try_from(name_and_signer: (String, &AnySigner)) -> Result<Self, Self::Error> {
        let (name, signer) = name_and_signer;
        let fingerprint = signer.fingerprint()?.to_string();
        let xpub = signer.xpub()?;
        let id = signer.identifier()?;

        Ok(Self {
            name,
            id,
            fingerprint,
            xpub,
        })
    }
}
