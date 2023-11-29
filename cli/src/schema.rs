use rpc_model::request::Direction;
use serde_json::Value;

use crate::args;

pub(crate) fn schema(a: args::SchemaArgs, client: app::Client) -> Result<Value, anyhow::Error> {
    Ok(match a.command {
        args::DirectionCommand::Request(b) => match b.command {
            args::MainCommand::Wallet(c) => match c.command {
                args::WalletSubCommandsEnum::Load => {
                    client.schema("load_wallet", Direction::Request)?
                }
                _ => todo!(),
            },
            args::MainCommand::Signer(_) => todo!(),
        },
        args::DirectionCommand::Response(_) => todo!(),
    })
}
