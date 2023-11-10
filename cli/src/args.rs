use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(ValueEnum, Clone, Debug)]
pub enum Network {
    Mainnet,
    Testnet,
    Regtest,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Network
    #[structopt(short, long, default_value = "testnet")]
    pub network: Network,

    /// Electrum URL
    #[structopt(short, long)]
    pub electrum_url: Option<String>,

    /// Writes to stderr instead of the default `debug.log`
    #[structopt(long)]
    pub stderr: bool,

    /// The sub command
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// server only
    Server(ServerArgs),
    /// signer
    Signer(SignerArgs),
    /// wallet
    Wallet(WalletArgs),

    WalletsList,
}

#[derive(Debug, Args)]
pub struct SignerArgs {
    #[command(subcommand)]
    pub command: SignerCommand,
}

#[derive(Debug, Subcommand)]
pub enum SignerCommand {
    Generate,
    Load {
        #[arg(long)]
        mnemonic: String,

        #[arg(long)]
        name: String,
    },
    List,
    Sign,
}

#[derive(Debug, Args)]
pub struct WalletArgs {
    /// Wallet name
    #[arg(short, long)]
    pub name: String,

    #[command(subcommand)]
    pub command: WalletCommand,
}

#[derive(Debug, Subcommand)]
pub enum WalletCommand {
    Load {
        descriptor: String,
    },
    Unload,
    Address {
        #[arg(long)]
        index: Option<u32>,
    },
    Balance,
    Tx,
}

#[derive(Debug, Args)]
pub struct ServerArgs {
    #[command(subcommand)]
    pub command: ServerCommand,
}

#[derive(Debug, Subcommand)]
pub enum ServerCommand {
    Start,
    Stop,
}
