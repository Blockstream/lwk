use std::str::FromStr;

use rpc_model::request::{self, Direction};
use schemars::schema_for;
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
                Method::Schema => todo!(),
                Method::GenerateSigner => todo!(),
                Method::Version => todo!(),
                Method::LoadWallet => schema_for!(request::LoadWallet),
                Method::UnloadWallet => todo!(),
                Method::ListWallets => todo!(),
                Method::LoadSigner => todo!(),
                Method::UnloadSigner => todo!(),
                Method::ListSigners => todo!(),
                Method::Address => todo!(),
                Method::Balance => todo!(),
                Method::SendMany => todo!(),
                Method::SinglesigDescriptor => todo!(),
                Method::MultisigDescriptor => todo!(),
                Method::Xpub => todo!(),
                Method::Sign => todo!(),
                Method::Broadcast => todo!(),
                Method::WalletDetails => todo!(),
                Method::WalletCombine => todo!(),
                Method::WalletPsetDetails => todo!(),
                Method::Issue => todo!(),
                Method::Contract => todo!(),
                Method::Stop => todo!(),
            },
            Direction::Response => match self {
                Method::Schema => todo!(),
                Method::GenerateSigner => todo!(),
                Method::Version => todo!(),
                Method::LoadWallet => todo!(),
                Method::UnloadWallet => todo!(),
                Method::ListWallets => todo!(),
                Method::LoadSigner => todo!(),
                Method::UnloadSigner => todo!(),
                Method::ListSigners => todo!(),
                Method::Address => todo!(),
                Method::Balance => todo!(),
                Method::SendMany => todo!(),
                Method::SinglesigDescriptor => todo!(),
                Method::MultisigDescriptor => todo!(),
                Method::Xpub => todo!(),
                Method::Sign => todo!(),
                Method::Broadcast => todo!(),
                Method::WalletDetails => todo!(),
                Method::WalletCombine => todo!(),
                Method::WalletPsetDetails => todo!(),
                Method::Issue => todo!(),
                Method::Contract => todo!(),
                Method::Stop => todo!(),
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
