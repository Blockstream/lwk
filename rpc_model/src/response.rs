use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An empty response.
#[derive(JsonSchema)]
pub struct Empty {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Version {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GenerateSigner {
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListSigners {
    pub signers: Vec<Signer>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Wallet {
    pub descriptor: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListWallets {
    pub wallets: Vec<Wallet>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnloadWallet {
    pub unloaded: Wallet,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnloadSigner {
    pub unloaded: Signer,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Signer {
    pub name: String,
    pub fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xpub: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Address {
    pub address: String,
    pub index: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Balance {
    pub balance: HashMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Pset {
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SinglesigDescriptor {
    pub descriptor: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MultisigDescriptor {
    pub descriptor: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Xpub {
    pub keyorigin_xpub: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Broadcast {
    pub txid: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Contract {
    pub entity: Entity,
    pub issuer_pubkey: String,
    pub name: String,
    pub precision: u8,
    pub ticker: String,
    pub version: u8,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    domain: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub fingerprint: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletDetails {
    #[serde(rename = "type")]
    pub type_: String,
    pub signers: Vec<SignerDetails>,
    pub warnings: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletCombine {
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletPsetDetails {
    pub has_signatures_from: Vec<SignerDetails>,
    pub missing_signatures_from: Vec<SignerDetails>,
    pub balance: HashMap<String, i64>,
    pub warnings: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetDetails {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Asset {
    pub asset_id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListAssets {
    pub assets: Vec<Asset>,
}
