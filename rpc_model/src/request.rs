use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadWallet {
    pub descriptor: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadWallet {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoadSigner {
    pub name: String,
    pub kind: String,
    pub mnemonic: Option<String>,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnloadSigner {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Address {
    pub name: String,
    pub index: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Balance {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Send {
    pub addressees: Vec<UnvalidatedAddressee>,
    pub fee_rate: Option<f32>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnvalidatedAddressee {
    /// The amount to send in satoshi
    pub satoshi: u64,

    /// The address to send to
    ///
    /// If "burn", the output will be burned
    pub address: String,

    /// The asset to send
    ///
    /// If empty, the policy asset
    pub asset: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SinglesigDescriptor {
    pub name: String,
    pub descriptor_blinding_key: String,
    pub singlesig_kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultisigDescriptor {
    pub descriptor_blinding_key: String,
    pub multisig_kind: String,
    pub threshold: u32,
    pub keyorigin_xpubs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Xpub {
    pub name: String,
    pub xpub_kind: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sign {
    pub name: String,
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Broadcast {
    pub name: String,
    pub dry_run: bool,
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletDetails {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub name: String,
    pub satoshi_asset: u64,
    pub address_asset: Option<String>,
    pub satoshi_token: u64,
    pub address_token: Option<String>,
    pub contract: Option<String>,
    pub fee_rate: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Contract {
    pub domain: String,
    pub issuer_pubkey: String,
    pub name: String,
    pub precision: u8,
    pub ticker: String,
    pub version: u8,
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
pub struct WalletCombine {
    pub name: String,
    pub pset: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletPsetDetails {
    pub name: String,
    pub pset: String,
}
