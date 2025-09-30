use lwk_app::{method::Method, Client};
use lwk_rpc_model::request::Direction;
use serde_json::Value;

use crate::args::{
    Amp2SubCommandsEnum, AssetSubCommandsEnum, DirectionCommand, MainCommand, SchemaArgs,
    ServerSubCommandsEnum, SignerSubCommandsEnum, WalletSubCommandsEnum,
};

pub(crate) fn schema(a: SchemaArgs, client: Client) -> Result<Value, anyhow::Error> {
    Ok(match a.command {
        DirectionCommand::Request(req) => match req.command {
            MainCommand::Server(w) => client.schema(w.command.into(), Direction::Request)?,
            MainCommand::Wallet(w) => client.schema(w.command.into(), Direction::Request)?,
            MainCommand::Signer(s) => client.schema(s.command.into(), Direction::Request)?,
            MainCommand::Asset(s) => client.schema(s.command.into(), Direction::Request)?,
            MainCommand::Amp2(s) => client.schema(s.command.into(), Direction::Request)?,
            MainCommand::Schema => client.schema(Method::Schema, Direction::Request)?,
        },
        DirectionCommand::Response(res) => match res.command {
            MainCommand::Server(w) => client.schema(w.command.into(), Direction::Response)?,
            MainCommand::Wallet(w) => client.schema(w.command.into(), Direction::Response)?,
            MainCommand::Signer(s) => client.schema(s.command.into(), Direction::Response)?,
            MainCommand::Asset(s) => client.schema(s.command.into(), Direction::Response)?,
            MainCommand::Amp2(s) => client.schema(s.command.into(), Direction::Response)?,
            MainCommand::Schema => client.schema(Method::Schema, Direction::Response)?,
        },
    })
}

impl From<ServerSubCommandsEnum> for Method {
    fn from(value: ServerSubCommandsEnum) -> Self {
        match value {
            ServerSubCommandsEnum::Scan => Method::Scan,
            ServerSubCommandsEnum::Stop => Method::Stop,
        }
    }
}

impl From<WalletSubCommandsEnum> for Method {
    fn from(value: WalletSubCommandsEnum) -> Self {
        match value {
            WalletSubCommandsEnum::Load => Method::WalletLoad,
            WalletSubCommandsEnum::Unload => Method::WalletUnload,
            WalletSubCommandsEnum::List => Method::WalletList,
            WalletSubCommandsEnum::Address => Method::WalletAddress,
            WalletSubCommandsEnum::Balance => Method::WalletBalance,
            WalletSubCommandsEnum::Send => Method::WalletSendMany,
            WalletSubCommandsEnum::Issue => Method::WalletIssue,
            WalletSubCommandsEnum::Reissue => Method::WalletReissue,
            WalletSubCommandsEnum::MultisigDesc => Method::WalletMultisigDescriptor,
            WalletSubCommandsEnum::Broadcast => Method::WalletBroadcast,
            WalletSubCommandsEnum::Details => Method::WalletDetails,
            WalletSubCommandsEnum::Combine => Method::WalletCombine,
            WalletSubCommandsEnum::PsetDetails => Method::WalletPsetDetails,
            WalletSubCommandsEnum::Utxos => Method::WalletUtxos,
            WalletSubCommandsEnum::Txs => Method::WalletTxs,
            WalletSubCommandsEnum::SetTxMemo => Method::WalletSetTxMemo,
            WalletSubCommandsEnum::SetAddrMemo => Method::WalletSetAddrMemo,
        }
    }
}

impl From<SignerSubCommandsEnum> for Method {
    fn from(value: SignerSubCommandsEnum) -> Self {
        match value {
            SignerSubCommandsEnum::Generate => Method::SignerGenerate,
            SignerSubCommandsEnum::JadeId => Method::SignerJadeId,
            SignerSubCommandsEnum::LoadSoftware => Method::SignerLoadSoftware,
            SignerSubCommandsEnum::LoadJade => Method::SignerLoadJade,
            SignerSubCommandsEnum::LoadExternal => Method::SignerLoadExternal,
            SignerSubCommandsEnum::Unload => Method::SignerUnload,
            SignerSubCommandsEnum::Details => Method::SignerDetails,
            SignerSubCommandsEnum::List => Method::SignerList,
            SignerSubCommandsEnum::Sign => Method::SignerSign,
            SignerSubCommandsEnum::SinglesigDesc => Method::SignerSinglesigDescriptor,
            SignerSubCommandsEnum::Xpub => Method::SignerXpub,
            SignerSubCommandsEnum::DeriveBip85 => Method::SignerDeriveBip85,
        }
    }
}

impl From<AssetSubCommandsEnum> for Method {
    fn from(value: AssetSubCommandsEnum) -> Self {
        match value {
            AssetSubCommandsEnum::Contract => Method::AssetContract,
            AssetSubCommandsEnum::Details => Method::AssetDetails,
            AssetSubCommandsEnum::List => Method::AssetList,
            AssetSubCommandsEnum::Insert => Method::AssetInsert,
            AssetSubCommandsEnum::Remove => Method::AssetRemove,
            AssetSubCommandsEnum::Publish => Method::AssetPublish,
        }
    }
}

impl From<Amp2SubCommandsEnum> for Method {
    fn from(value: Amp2SubCommandsEnum) -> Self {
        match value {
            Amp2SubCommandsEnum::Descriptor => Method::Amp2Descriptor,
            Amp2SubCommandsEnum::Register => Method::Amp2Register,
            Amp2SubCommandsEnum::Cosign => Method::Amp2Cosign,
        }
    }
}
