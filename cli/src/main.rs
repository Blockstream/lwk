use std::fs::File;

use anyhow::Context;
use app::config::Config;
use clap::Parser;
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

use crate::args::{CliCommand, Network};

mod args;

fn main() -> anyhow::Result<()> {
    let args = args::Cli::parse();
    // config
    // - network
    // - config file
    // - json rpc host/port
    // - electrum server
    // - file/directory path

    // set up logging
    let file = File::options()
        .create(true)
        .append(true)
        .open("debug.log")?;
    let (appender, _guard) = tracing_appender::non_blocking(file);
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_writer(appender)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    tracing::info!("CLI initialized with args: {:?}", args);

    // start the app with default host/port
    let config = match args.network {
        Network::Mainnet => Config::default_mainnet(),
        Network::Testnet => Config::default_testnet(),
        Network::Regtest => Config::default_regtest(&args.electrum_url),
    };
    let mut app = app::App::new(config)?;
    // get a client to make requests
    let client = app.client()?;

    match args.command {
        CliCommand::Server => {
            app.run().with_context(|| {
                format!(
                    "Cannot start the server at \"{}\". It is probably already running.",
                    app.addr()
                )
            })?;
            // get the app version
            let version = client.version()?.version;
            tracing::info!("App running version {}", version);

            app.join_threads()?;
        }
        CliCommand::Signer(a) => match a.command {
            args::SignerCommand::Generate => {
                let j = client.generate_signer()?;
                println!("{}", serde_json::to_string(&j)?);
            }
        },
        CliCommand::Wallet(_) => todo!(),
    }

    Ok(())
}
