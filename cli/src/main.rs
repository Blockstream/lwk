use clap::Parser;
use cli::inner_main;
use cli::Cli;

mod args;

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    // config
    // - network
    // - config file
    // - json rpc host/port
    // - electrum server
    // - file/directory path

    let value = match inner_main(args) {
        Ok(value) => value,
        Err(e) => match e.downcast() {
            Ok(e) => match e {
                app::Error::RpcError(e) => serde_json::to_value(&e)?,
                e => return Err(e.into()),
            },
            Err(e) => return Err(e),
        },
    };
    println!("{:#}", value);
    Ok(())
}
