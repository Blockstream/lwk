use std::fs::File;

use app::config::Config;
use console::Style;
// use dialoguer::{theme::ColorfulTheme, Input};
use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run the application as JSON RPC server only.
    #[arg(short, long)]
    server: bool,
}

//
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    // config
    // - json rpc host/port

    // set up logging
    let file = File::options()
        .create(true)
        .append(true)
        .open("debug.log")?;
    let (appender, _guard) = tracing_appender::non_blocking(file);
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(appender)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    tracing::info!("CLI initialized with args: {:?}", args);

    // start the app with default host/port
    let app = app::App::new(Config::default())?;
    // get a client to make requests
    let client = app.client()?;
    // get the app version
    let version = client.version()?;

    if args.server {
        tracing::info!("Server mode. Version {}", version);
        // todo: run as server only
        // todo: interrupt handling
        todo!();
    } else {
        // run the interactive CLI
        let bg = Style::new().black().on_cyan();
        let text = format!("Liquid MultiSig Hardware Wallet CLI (version {})", version);
        println!("{}", bg.apply_to(text));
        // todo: cli loop
        println!("wip. exiting");
    }

    Ok(())
}
