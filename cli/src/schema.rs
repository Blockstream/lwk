use app::method::Method;
use rpc_model::request::Direction;
use serde_json::Value;

use crate::args::{self, SignerSubCommandsEnum, WalletSubCommandsEnum};

pub(crate) fn schema(a: args::SchemaArgs, client: app::Client) -> Result<Value, anyhow::Error> {
    Ok(match a.command {
        args::DirectionCommand::Request(req) => match req.command {
            args::MainCommand::Wallet(w) => client.schema(w.command.into(), Direction::Request)?,
            args::MainCommand::Signer(s) => client.schema(s.command.into(), Direction::Request)?,
        },
        args::DirectionCommand::Response(res) => match res.command {
            args::MainCommand::Wallet(w) => client.schema(w.command.into(), Direction::Response)?,
            args::MainCommand::Signer(s) => client.schema(s.command.into(), Direction::Response)?,
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
            WalletSubCommandsEnum::Contract => Method::Contract,
            WalletSubCommandsEnum::Issue => Method::Issue,
            WalletSubCommandsEnum::Issuances => todo!(),
            WalletSubCommandsEnum::Reissue => todo!(),
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
            SignerSubCommandsEnum::Load => Method::LoadSigner,
            SignerSubCommandsEnum::Unload => Method::UnloadSigner,
            SignerSubCommandsEnum::List => Method::ListSigners,
            SignerSubCommandsEnum::Sign => Method::Sign,
            SignerSubCommandsEnum::SinglesigDesc => Method::SinglesigDescriptor,
            SignerSubCommandsEnum::Xpub => Method::Xpub,
        }
    }
}
