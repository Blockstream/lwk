use clap::Parser;
use lwk_app::Error;
use lwk_cli::{inner_main, Cli};

mod args;

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let value = match inner_main(args) {
        Ok(value) => value,
        Err(e) => {
            if let Some(Error::RpcError(e)) = e.downcast_ref::<Error>() {
                serde_json::to_value(e)?
            } else {
                return Err(e);
            }
        }
    };
    println!("{value:#}");
    Ok(())
}
