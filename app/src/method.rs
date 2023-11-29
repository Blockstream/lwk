use std::str::FromStr;

use rpc_model::{
    request::{self, Direction},
    response,
};
use schemars::{schema::RootSchema, schema_for};
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
#[error("The rpc method '{name}' does not exist")]
pub struct MethodNotExist {
    name: String,
}
pub(crate) enum Method {
    Schema,
    GenerateSigner,
    Version,
    LoadWallet,
    UnloadWallet,
    ListWallets,
    LoadSigner,
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
    Contract,
    Stop,
}
impl Method {
    pub(crate) fn schema(
        &self,
        direction: rpc_model::request::Direction,
    ) -> Result<Value, serde_json::Error> {
        serde_json::to_value(match direction {
            Direction::Request => match self {
                Method::Schema => schema_for!(request::Schema),
                Method::GenerateSigner => RootSchema::default(),
                Method::Version => RootSchema::default(),
                Method::LoadWallet => schema_for!(request::LoadWallet),
                Method::UnloadWallet => schema_for!(request::UnloadWallet),
                Method::ListWallets => RootSchema::default(),
                Method::LoadSigner => schema_for!(request::LoadSigner),
                Method::UnloadSigner => schema_for!(request::UnloadSigner),
                Method::ListSigners => RootSchema::default(),
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
                Method::Contract => schema_for!(request::Contract),
                Method::Stop => RootSchema::default(),
            },
            Direction::Response => match self {
                Method::Schema => RootSchema::default(),
                Method::GenerateSigner => schema_for!(response::GenerateSigner),
                Method::Version => schema_for!(response::Version),
                Method::LoadWallet => schema_for!(response::Wallet),
                Method::UnloadWallet => schema_for!(response::UnloadWallet),
                Method::ListWallets => schema_for!(response::ListWallets),
                Method::LoadSigner => todo!(),
                Method::UnloadSigner => todo!(),
                Method::ListSigners => todo!(),
                Method::Address => todo!(),
                Method::Balance => schema_for!(response::Balance),
                Method::SendMany => schema_for!(response::Pset),
                Method::SinglesigDescriptor => schema_for!(response::SinglesigDescriptor),
                Method::MultisigDescriptor => schema_for!(response::MultisigDescriptor),
                Method::Xpub => schema_for!(response::Xpub),
                Method::Sign => schema_for!(response::Pset),
                Method::Broadcast => todo!(),
                Method::WalletDetails => todo!(),
                Method::WalletCombine => schema_for!(response::WalletCombine),
                Method::WalletPsetDetails => todo!(),
                Method::Issue => schema_for!(response::Pset),
                Method::Contract => schema_for!(response::Contract),
                Method::Stop => RootSchema::default(),
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
            "load_signer" => Method::LoadSigner,
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
            "contract" => Method::Contract,
            "stop" => Method::Stop,
            _ => {
                return Err(MethodNotExist {
                    name: s.to_string(),
                })
            }
        })
    }
}
