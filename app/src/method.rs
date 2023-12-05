use std::str::FromStr;

use rpc_model::{
    request::{self, Direction},
    response,
};
use schemars::schema_for;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
#[error("The rpc method '{name}' does not exist")]
pub struct MethodNotExist {
    name: String,
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(test, derive(enum_iterator::Sequence))]
pub enum Method {
    Schema,
    GenerateSigner,
    Version,
    LoadWallet,
    UnloadWallet,
    ListWallets,
    SignerLoadSoftware,
    SignerLoadJade,
    SignerLoadExternal,
    UnloadSigner,
    ListSigners,
    Address,
    Balance,
    SendMany,
    SinglesigDescriptor,
    MultisigDescriptor,
    Xpub,
    Sign,
    Broadcast,
    WalletDetails,
    WalletCombine,
    WalletPsetDetails,
    Issue,
    Reissue,
    Contract,
    AssetDetails,
    ListAssets,
    AssetInsert,
    AssetRemove,
    Stop,
    SignerJadeId,
}
impl Method {
    pub(crate) fn schema(&self, direction: request::Direction) -> Result<Value, serde_json::Error> {
        serde_json::to_value(match direction {
            Direction::Request => match self {
                Method::Schema => schema_for!(request::Schema),
                Method::GenerateSigner => schema_for!(request::Empty),
                Method::Version => schema_for!(request::Empty),
                Method::LoadWallet => schema_for!(request::LoadWallet),
                Method::UnloadWallet => schema_for!(request::UnloadWallet),
                Method::ListWallets => schema_for!(request::Empty),
                Method::SignerLoadSoftware => schema_for!(request::SignerLoadSoftware),
                Method::SignerLoadJade => schema_for!(request::SignerLoadJade),
                Method::SignerLoadExternal => schema_for!(request::SignerLoadExternal),
                Method::UnloadSigner => schema_for!(request::UnloadSigner),
                Method::ListSigners => schema_for!(request::Empty),
                Method::Address => schema_for!(request::Address),
                Method::Balance => schema_for!(request::Balance),
                Method::SendMany => schema_for!(request::Send),
                Method::SinglesigDescriptor => schema_for!(request::SinglesigDescriptor),
                Method::MultisigDescriptor => schema_for!(request::MultisigDescriptor),
                Method::Xpub => schema_for!(request::Xpub),
                Method::Sign => schema_for!(request::Sign),
                Method::Broadcast => schema_for!(request::Broadcast),
                Method::WalletDetails => schema_for!(request::WalletDetails),
                Method::WalletCombine => schema_for!(request::WalletCombine),
                Method::WalletPsetDetails => schema_for!(request::WalletPsetDetails),
                Method::Issue => schema_for!(request::Issue),
                Method::Reissue => schema_for!(request::Reissue),
                Method::Contract => schema_for!(request::Contract),
                Method::AssetDetails => schema_for!(request::AssetDetails),
                Method::ListAssets => schema_for!(request::Empty),
                Method::AssetInsert => schema_for!(request::AssetInsert),
                Method::AssetRemove => schema_for!(request::AssetRemove),
                Method::Stop => schema_for!(request::Empty),
                Method::SignerJadeId => schema_for!(request::Empty),
            },
            Direction::Response => match self {
                Method::Schema => return serde_json::from_str(include_str!("../schema.json")),
                Method::GenerateSigner => schema_for!(response::GenerateSigner),
                Method::Version => schema_for!(response::Version),
                Method::LoadWallet => schema_for!(response::Wallet),
                Method::UnloadWallet => schema_for!(response::UnloadWallet),
                Method::ListWallets => schema_for!(response::ListWallets),
                Method::SignerLoadSoftware => schema_for!(response::Signer),
                Method::SignerLoadJade => schema_for!(response::Signer),
                Method::SignerLoadExternal => schema_for!(response::Signer),
                Method::UnloadSigner => schema_for!(response::UnloadSigner),
                Method::ListSigners => schema_for!(response::ListSigners),
                Method::Address => schema_for!(response::Address),
                Method::Balance => schema_for!(response::Balance),
                Method::SendMany => schema_for!(response::Pset),
                Method::SinglesigDescriptor => schema_for!(response::SinglesigDescriptor),
                Method::MultisigDescriptor => schema_for!(response::MultisigDescriptor),
                Method::Xpub => schema_for!(response::Xpub),
                Method::Sign => schema_for!(response::Pset),
                Method::Broadcast => schema_for!(response::Broadcast),
                Method::WalletDetails => schema_for!(response::WalletDetails),
                Method::WalletCombine => schema_for!(response::WalletCombine),
                Method::WalletPsetDetails => schema_for!(response::WalletPsetDetails),
                Method::Issue => schema_for!(response::Pset),
                Method::Reissue => schema_for!(response::Pset),
                Method::Contract => schema_for!(response::Contract),
                Method::AssetDetails => schema_for!(response::AssetDetails),
                Method::ListAssets => schema_for!(response::ListAssets),
                Method::AssetInsert => schema_for!(response::Empty),
                Method::AssetRemove => schema_for!(request::Empty),
                Method::Stop => schema_for!(request::Empty),
                Method::SignerJadeId => schema_for!(response::JadeId),
            },
        })
    }
}

impl FromStr for Method {
    type Err = MethodNotExist;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "schema" => Method::Schema,
            "generate_signer" => Method::GenerateSigner,
            "version" => Method::Version,
            "load_wallet" => Method::LoadWallet,
            "unload_wallet" => Method::UnloadWallet,
            "list_wallets" => Method::ListWallets,
            "signer_load_software" => Method::SignerLoadSoftware,
            "signer_load_jade" => Method::SignerLoadJade,
            "signer_load_external" => Method::SignerLoadExternal,
            "unload_signer" => Method::UnloadSigner,
            "list_signers" => Method::ListSigners,
            "address" => Method::Address,
            "balance" => Method::Balance,
            "send_many" => Method::SendMany,
            "singlesig_descriptor" => Method::SinglesigDescriptor,
            "multisig_descriptor" => Method::MultisigDescriptor,
            "xpub" => Method::Xpub,
            "sign" => Method::Sign,
            "broadcast" => Method::Broadcast,
            "wallet_details" => Method::WalletDetails,
            "wallet_combine" => Method::WalletCombine,
            "wallet_pset_details" => Method::WalletPsetDetails,
            "issue" => Method::Issue,
            "reissue" => Method::Reissue,
            "contract" => Method::Contract,
            "asset_details" => Method::AssetDetails,
            "list_assets" => Method::ListAssets,
            "asset_insert" => Method::AssetInsert,
            "asset_remove" => Method::AssetRemove,
            "signer_jade_id" => Method::SignerJadeId,
            "stop" => Method::Stop,
            _ => {
                return Err(MethodNotExist {
                    name: s.to_string(),
                })
            }
        })
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Method::Schema => "schema",
            Method::GenerateSigner => "generate_signer",
            Method::Version => "version",
            Method::LoadWallet => "load_wallet",
            Method::UnloadWallet => "unload_wallet",
            Method::ListWallets => "list_wallets",
            Method::SignerLoadSoftware => "signer_load_software",
            Method::SignerLoadJade => "signer_load_jade",
            Method::SignerLoadExternal => "signer_load_external",
            Method::UnloadSigner => "unload_signer",
            Method::ListSigners => "list_signers",
            Method::Address => "address",
            Method::Balance => "balance",
            Method::SendMany => "send_many",
            Method::SinglesigDescriptor => "singlesig_descriptor",
            Method::MultisigDescriptor => "multisig_descriptor",
            Method::Xpub => "xpub",
            Method::Sign => "sign",
            Method::Broadcast => "broadcast",
            Method::WalletDetails => "wallet_details",
            Method::WalletCombine => "wallet_combine",
            Method::WalletPsetDetails => "wallet_pset_details",
            Method::Issue => "issue",
            Method::Reissue => "reissue",
            Method::Contract => "contract",
            Method::AssetDetails => "asset_details",
            Method::ListAssets => "list_assets",
            Method::AssetInsert => "asset_insert",
            Method::AssetRemove => "asset_remove",
            Method::Stop => "stop",
            Method::SignerJadeId => "signer_jade_id",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod test {
    use enum_iterator::all;

    use super::Method;

    #[test]
    fn method_roundtrip() {
        let all = all::<Method>().collect::<Vec<_>>();
        for m in all {
            assert_eq!(m, m.to_string().parse().unwrap())
        }
    }
}
