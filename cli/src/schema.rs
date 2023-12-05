use app::method::Method;
use rpc_model::request::Direction;
use serde_json::Value;

use crate::args::{self, AssetSubCommandsEnum, SignerSubCommandsEnum, WalletSubCommandsEnum};

pub(crate) fn schema(a: args::SchemaArgs, client: app::Client) -> Result<Value, anyhow::Error> {
    Ok(match a.command {
        args::DirectionCommand::Request(req) => match req.command {
            args::MainCommand::Wallet(w) => client.schema(w.command.into(), Direction::Request)?,
            args::MainCommand::Signer(s) => client.schema(s.command.into(), Direction::Request)?,
            args::MainCommand::Asset(s) => client.schema(s.command.into(), Direction::Request)?,
            args::MainCommand::Schema => client.schema(Method::Schema, Direction::Request)?,
        },
        args::DirectionCommand::Response(res) => match res.command {
            args::MainCommand::Wallet(w) => client.schema(w.command.into(), Direction::Response)?,
            args::MainCommand::Signer(s) => client.schema(s.command.into(), Direction::Response)?,
            args::MainCommand::Asset(s) => client.schema(s.command.into(), Direction::Response)?,
            args::MainCommand::Schema => client.schema(Method::Schema, Direction::Response)?,
        },
    })
}

impl From<WalletSubCommandsEnum> for Method {
    fn from(value: WalletSubCommandsEnum) -> Self {
        match value {
            WalletSubCommandsEnum::Load => Method::LoadWallet,
            WalletSubCommandsEnum::Unload => Method::UnloadWallet,
            WalletSubCommandsEnum::List => Method::ListWallets,
            WalletSubCommandsEnum::Address => Method::Address,
            WalletSubCommandsEnum::Balance => Method::Balance,
            WalletSubCommandsEnum::Send => Method::SendMany,
            WalletSubCommandsEnum::Issue => Method::Issue,
            WalletSubCommandsEnum::Reissue => Method::Reissue,
            WalletSubCommandsEnum::MultisigDesc => Method::MultisigDescriptor,
            WalletSubCommandsEnum::Broadcast => Method::Broadcast,
            WalletSubCommandsEnum::Details => Method::WalletDetails,
            WalletSubCommandsEnum::Combine => Method::WalletCombine,
            WalletSubCommandsEnum::PsetDetails => Method::WalletPsetDetails,
        }
    }
}

impl From<SignerSubCommandsEnum> for Method {
    fn from(value: SignerSubCommandsEnum) -> Self {
        match value {
            SignerSubCommandsEnum::Generate => Method::GenerateSigner,
            SignerSubCommandsEnum::JadeId => Method::SignerJadeId,
            SignerSubCommandsEnum::LoadSoftware => Method::SignerLoadSoftware,
            SignerSubCommandsEnum::LoadJade => Method::SignerLoadJade,
            SignerSubCommandsEnum::LoadExternal => Method::SignerLoadExternal,
            SignerSubCommandsEnum::Unload => Method::UnloadSigner,
            SignerSubCommandsEnum::List => Method::ListSigners,
            SignerSubCommandsEnum::Sign => Method::Sign,
            SignerSubCommandsEnum::SinglesigDesc => Method::SinglesigDescriptor,
            SignerSubCommandsEnum::Xpub => Method::Xpub,
        }
    }
}

impl From<AssetSubCommandsEnum> for Method {
    fn from(value: AssetSubCommandsEnum) -> Self {
        match value {
            AssetSubCommandsEnum::Contract => Method::Contract,
            AssetSubCommandsEnum::Details => Method::AssetDetails,
            AssetSubCommandsEnum::List => Method::ListAssets,
            AssetSubCommandsEnum::Insert => Method::AssetInsert,
            AssetSubCommandsEnum::Remove => Method::AssetRemove,
        }
    }
}
