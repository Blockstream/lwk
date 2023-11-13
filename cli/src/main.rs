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

    let value = inner_main(args)?;
    println!("{value:#}");
    Ok(())
}
