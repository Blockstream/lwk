use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An empty request, doesn't require any param.
#[derive(JsonSchema)]
pub struct Empty {}

/// Request a JSON schema of a method of the RPC
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Schema {
    /// Name of the method to request the schema for
    pub method: String,

    /// Specify if requesting the schema for the request or the response
    pub direction: Direction,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Request,
    Response,
}

/// Load a wallet in the server
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LoadWallet {
    /// The read-only descriptor describing the wallet outputs
    pub descriptor: String,

    /// The name given to the wallet, will be needed for calls related to the wallet
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnloadWallet {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LoadSigner {
    pub name: String,
    pub kind: String,
    pub mnemonic: Option<String>,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UnloadSigner {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Address {
    pub name: String,
    pub index: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Balance {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Send {
    pub addressees: Vec<UnvalidatedAddressee>,
    pub fee_rate: Option<f32>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SinglesigDescriptor {
    pub name: String,
    pub descriptor_blinding_key: String,
    pub singlesig_kind: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MultisigDescriptor {
    pub descriptor_blinding_key: String,
    pub multisig_kind: String,
    pub threshold: u32,
    pub keyorigin_xpubs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Xpub {
    pub name: String,
    pub xpub_kind: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Sign {
    pub name: String,
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Broadcast {
    pub name: String,
    pub dry_run: bool,
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletDetails {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    pub name: String,
    pub satoshi_asset: u64,
    pub address_asset: Option<String>,
    pub satoshi_token: u64,
    pub address_token: Option<String>,
    pub contract: Option<String>,
    pub fee_rate: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Contract {
    pub domain: String,
    pub issuer_pubkey: String,
    pub name: String,
    pub precision: u8,
    pub ticker: String,
    pub version: u8,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletCombine {
    pub name: String,
    pub pset: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WalletPsetDetails {
    pub name: String,
    pub pset: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssetDetails {
    pub asset_id: String,
}

#[cfg(test)]
mod test {
    use schemars::schema_for;

    use crate::request::*;

    #[test]
    fn test_json_schema() {
        let schema = schema_for!(LoadWallet);
        assert_eq!(
            r#"{"$schema":"http://json-schema.org/draft-07/schema#","title":"LoadWallet","description":"Load a wallet in the server","type":"object","required":["descriptor","name"],"properties":{"descriptor":{"description":"The read-only descriptor describing the wallet outputs","type":"string"},"name":{"description":"The name given to the wallet, will be needed for calls related to the wallet","type":"string"}}}"#,
            serde_json::to_string(&schema).unwrap()
        );
    }
}
