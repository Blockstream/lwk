use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Mainnet
    #[structopt(short, long)]
    pub mainnet: bool,

    /// Testnet
    #[structopt(short, long)]
    pub testnet: bool,

    /// Electrum URL
    #[structopt(short, long, default_value = "")]
    pub electrum_url: String,

    /// The sub command
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// server only
    Server,
    /// signer
    Signer(SignerArgs),
    /// wallet
    Wallet(WalletArgs),
}

#[derive(Debug, Args)]
pub struct SignerArgs {
    #[command(subcommand)]
    pub command: SignerCommand,
}

#[derive(Debug, Subcommand)]
pub enum SignerCommand {
    Generate,
}

#[derive(Debug, Args)]
pub struct WalletArgs {
    #[command(subcommand)]
    pub command: WalletCommand,
}

#[derive(Debug, Subcommand)]
pub enum WalletCommand {
    Balance,
}
