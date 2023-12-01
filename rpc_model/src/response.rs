//! Data models of every response made via RPC

#[cfg(doc)]
use crate::request;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An empty response.
#[derive(JsonSchema)]
pub struct Empty {}

/// Server version response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Version {
    /// The server version
    pub version: String,
}

/// Response for generate signer
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GenerateSigner {
    /// Randomly generated mnemonic from the server
    pub mnemonic: String,
}

/// Response for list signers call
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListSigners {
    /// Returned signers currently loaded in the server
    pub signers: Vec<Signer>,
}

/// Wallet response, returned from various call such as [`request::LoadWallet`], [`request::UnloadWallet`]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Wallet {
    /// Public descriptor definining wallet outputs
    pub descriptor: String,

    /// The wallet name
    pub name: String,
}

/// Response for list wallets call
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListWallets {
    /// Returned wallets currently loaded in the server
    pub wallets: Vec<Wallet>,
}

/// Response for unload wallet call
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnloadWallet {
    /// Details of the wallet unloaded from the server
    pub unloaded: Wallet,
}

/// Response for unload signer call
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnloadSigner {
    /// Details of the signer unloaded from the server
    pub unloaded: Signer,
}

/// Response of a signer
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Signer {
    /// The signer name
    pub name: String,

    /// The fingerprint of the signer, 4 bytes returned as 8 hex characters
    pub fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]

    /// Full identifier of the signer, of which the fingerprint is a subset
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]

    /// Master xpub of the signer
    pub xpub: Option<String>,
}

/// Address response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Address {
    /// The receiving address
    pub address: String,

    /// The index of the derivation of the given address
    pub index: u32,
}

/// Balance respone
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Balance {
    /// A map of the balance of every asset in the wallet
    pub balance: HashMap<String, u64>,
}

/// PSET response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Pset {
    /// The PSET in base64 format
    pub pset: String,
}

/// Response containing a single signature descriptor
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SinglesigDescriptor {
    /// The singlesig descriptor
    pub descriptor: String,
}

/// Response containing a multi signature descriptor
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MultisigDescriptor {
    /// The multisig descriptor
    pub descriptor: String,
}

/// A response containing an xpub with keyorigin
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Xpub {
    /// The xpub with keyorigin prepended (fingerprint+derivation path)
    pub keyorigin_xpub: String,
}

/// The response of a broadcast
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Broadcast {
    /// The txid of the transaction just broadacasted
    pub txid: String,
}

/// A response of a JSON contract containing asset metadata and validated according to the contract rules
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Contract {
    /// Entity emitting the asset
    pub entity: Entity,

    /// Pubkey of the asset issuer, in the 33 bytes format expressed 66 hex chars
    pub issuer_pubkey: String,

    /// Name of the asset
    pub name: String,

    /// Precision of the asset
    pub precision: u8,

    /// Ticker of the asset
    pub ticker: String,

    /// Version of the contract
    pub version: u8,
}

/// Entity issuing the asset
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    /// Domain of the entity issuing the asset
    domain: String,
}

/// Details of a signer
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignerDetails {
    /// The name of the signer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// The fingerprint of the signer
    pub fingerprint: String,
}

/// Details of a wallet
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletDetails {
    /// Type of the wallet // TODO make enum
    #[serde(rename = "type")]
    pub type_: String,

    /// Signers of this wallet
    pub signers: Vec<SignerDetails>,

    /// Warnings on this wallet
    pub warnings: String,
}

/// Response to wallet combine
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletCombine {
    /// PSET in base64 format
    pub pset: String,
}

/// Response containing detail of a PSET
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Issuance {
    /// Asset id
    pub asset: String,

    /// Token id
    pub token: String,

    /// Wheter the issuance is confidential
    pub is_confidential: bool,

    /// Index of the input containing the issuance
    pub vin: u32,

    /// Number of units of the asset
    pub asset_satoshi: u64,

    /// Number of reissuance token
    pub token_satoshi: u64,
}

/// Details about a reissuance
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Reissuance {
    /// The asset id
    pub asset: String,

    /// The token id,
    pub token: String,

    /// Wheter the reissuance is confidential
    pub is_confidential: bool,

    /// Index of the input containing the reissuance
    pub vin: u32,

    /// Number of units of the asset reissued
    pub asset_satoshi: u64,
}

/// Details of a PSET
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletPsetDetails {
    /// Signatures contained in the PSET
    pub has_signatures_from: Vec<SignerDetails>,

    /// Signature required to spend but missing in the PSET
    pub missing_signatures_from: Vec<SignerDetails>,

    /// Net balance of the assets for the point of view of the given wallet
    pub balance: HashMap<String, i64>,

    /// Fee of the transaction
    pub fee: u64,

    /// Issuances contained in the PSET
    pub issuances: Vec<Issuance>,

    /// Reissuance contained in the PSET
    pub reissuances: Vec<Reissuance>,

    /// Warnings
    pub warnings: String,
}

/// Details of an asset
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetDetails {
    /// Name of the asset
    pub name: String,
}

/// Asset details
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Asset {
    /// The asset identifier (32 bytes as 64 hex chars)
    pub asset_id: String,

    /// The name of the asset
    pub name: String,
}

/// A list of assets
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListAssets {
    /// The list of assets
    pub assets: Vec<Asset>,
}

/// The wallet type // TODO move to response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub enum WalletType {
    /// Unknowm type
    Unknown,

    /// Witness pay to public key hash (segwit)
    Wpkh,

    /// Script hash Witness pay to public key hash (nested segwit)
    ShWpkh,

    /// Witnes script hash, multisig N of M
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