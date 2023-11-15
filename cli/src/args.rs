use std::fmt::Display;

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

    /// Electrum URL, if not specified a reasonable default is specified according to the network
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
    /// To start and stop the server
    Server(ServerArgs),
    /// Signer related commands (generate, load, list, sign...)
    Signer(SignerArgs),
    /// Wallet related commands (load, list, balance, address, tx...)
    Wallet(WalletArgs),
}

#[derive(Debug, Args)]
pub struct SignerArgs {
    #[command(subcommand)]
    pub command: SignerCommand,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum SignerKind {
    Software,
    Serial,
}

impl std::fmt::Display for SignerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SignerKind::Software => write!(f, "software"),
            SignerKind::Serial => write!(f, "serial"),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum SignerCommand {
    /// Generate a softwawre signer, returns a mnemonic
    Generate,

    /// Load a software signer from a mnemonic giving it a name
    Load {
        #[arg(long)]
        name: String,

        #[arg(long)]
        kind: SignerKind,

        #[arg(long)]
        mnemonic: Option<String>, // TODO is it right to have the mnemonic as arg?
    },

    /// Unload a software signer
    Unload {
        #[arg(long)]
        name: String,
    },

    /// List loaded signers
    List,

    /// Sign a transaction
    Sign,

    ///  Prints a singlesig descriptor using this signer key
    SinglesigDescriptor {
        #[arg(long)]
        name: String,

        #[arg(long)]
        descriptor_blinding_key: BlindingKeyKind,

        #[arg(long)]
        kind: SinglesigKind,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum BlindingKeyKind {
    Slip77,
    View,
    Bare,
}

impl Display for BlindingKeyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlindingKeyKind::Slip77 => write!(f, "slip77"),
            BlindingKeyKind::View => write!(f, "view"),
            BlindingKeyKind::Bare => write!(f, "bare"),
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
pub enum SinglesigKind {
    Wpkh,
    Shwpkh,
}

impl Display for SinglesigKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinglesigKind::Wpkh => write!(f, "wpkh"),
            SinglesigKind::Shwpkh => write!(f, "shwpkh"),
        }
    }
}

#[derive(Debug, Args)]
pub struct WalletArgs {
    #[command(subcommand)]
    pub command: WalletCommand,
}

#[derive(Debug, Subcommand)]
pub enum WalletCommand {
    /// Load a wallet with a CT descriptor giving it a name
    Load {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        descriptor: String,
    },

    /// Unload a wallet
    Unload {
        /// Wallet name
        #[arg(short, long)]
        name: String,
    },

    /// List existing loaded wallets
    List,

    /// Get an address from the given wallet name
    Address {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        #[arg(long)]
        index: Option<u32>,
    },

    /// Get the balance of the given wallet name
    Balance {
        /// Wallet name
        #[arg(short, long)]
        name: String,
    },

    /// Create an unsigned transaction (PSET) (send, issue, reissue, burn)
    Send {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        /// Specify a recipient in the form "address:satoshi:asset_id"
        ///
        /// Can be specified multiple times.
        #[arg(long, required = true)]
        recipient: Vec<String>,

        /// Fee rate to use
        #[arg(long)]
        fee_rate: Option<f32>,
    },

    Issue {},
    Reissue {},
}

#[derive(Debug, Args)]
pub struct ServerArgs {
    #[command(subcommand)]
    pub command: ServerCommand,
}

#[derive(Debug, Subcommand)]
pub enum ServerCommand {
    /// Start the server
    Start,

    /// Stop the server, could be stopped also with SIGINT (ctrl-c)
    Stop,
}
