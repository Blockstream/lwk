use std::{fmt::Display, net::SocketAddr, path::PathBuf};

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

    /// The sub command
    #[command(subcommand)]
    pub command: CliCommand,

    /// Where the log file and other data goes.
    #[arg(long, default_value = "/tmp/.ks")]
    pub datadir: PathBuf,

    /// If launching the server is where it listens, otherwise is where the client connects to.
    #[arg(long, default_value = "127.0.0.1:32111")]
    pub addr: SocketAddr,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// To start and stop the server
    Server(ServerArgs),

    /// Signer related commands (generate, load, list, sign...)
    Signer(SignerArgs),

    /// Wallet related commands (load, list, balance, address, tx...)
    Wallet(WalletArgs),

    /// Asset related commands
    Asset(AssetArgs),

    /// Returns JSON schema of a RPC request/response for a given command.
    ///
    /// For example if you want a description of the fields returned by `wallet list` use `schema response wallet list`
    Schema(SchemaArgs),
}

#[derive(Debug, Args)]
pub struct SignerArgs {
    #[command(subcommand)]
    pub command: SignerCommand,
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    #[command(subcommand)]
    pub command: DirectionCommand,
}

#[derive(Debug, Subcommand)]
pub enum DirectionCommand {
    /// Schemas for requests to the server
    Request(MainCommandArgs),

    /// Schemas for responses from the server
    Response(MainCommandArgs),
}

#[derive(Debug, Args)]
pub struct MainCommandArgs {
    #[command(subcommand)]
    pub command: MainCommand,
}

#[derive(Debug, Subcommand)]
#[clap(disable_help_flag = true, disable_help_subcommand = true)]
pub enum MainCommand {
    Wallet(WalletSubCommands),
    Signer(SignerSubCommands),
    Asset(AssetSubCommands),
    Schema,
}

#[derive(Debug, Args)]
pub struct WalletSubCommands {
    #[command(subcommand)]
    pub command: WalletSubCommandsEnum,
}

#[derive(Debug, Subcommand, ValueEnum, Clone)]
pub enum WalletSubCommandsEnum {
    Load,
    Unload,
    List,
    Address,
    Balance,
    Send,
    Issue,
    Issuances,
    Reissue,
    MultisigDesc,
    Broadcast,
    Details,
    Combine,
    PsetDetails,
}

#[derive(Debug, Args)]
pub struct SignerSubCommands {
    #[command(subcommand)]
    pub command: SignerSubCommandsEnum,
}

#[derive(Debug, Subcommand, ValueEnum, Clone)]
pub enum SignerSubCommandsEnum {
    Generate,
    LoadSoftware,
    LoadJade,
    LoadExternal,
    Unload,
    List,
    Sign,
    SinglesigDesc,
    Xpub,
}

#[derive(Debug, Args)]
pub struct AssetSubCommands {
    #[command(subcommand)]
    pub command: AssetSubCommandsEnum,
}

#[derive(Debug, Subcommand, ValueEnum, Clone)]
pub enum AssetSubCommandsEnum {
    Contract,
    Details,
    List,
    Insert,
    Remove,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum SignerKind {
    Software,
    Serial,
    External,
}

impl std::fmt::Display for SignerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SignerKind::Software => write!(f, "software"),
            SignerKind::Serial => write!(f, "serial"),
            SignerKind::External => write!(f, "external"),
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
pub enum XpubKind {
    Bip84,
}

impl std::fmt::Display for XpubKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            XpubKind::Bip84 => write!(f, "bip84"),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum SignerCommand {
    /// Generate a softwawre signer, returns a mnemonic
    Generate,

    /// Probe connected Jades, unlocks and returns identifiers that allows to load a Jade
    JadeId {
        /// The socket address to connect to jade emulator
        #[arg(long)]
        emulator: Option<SocketAddr>,
    },

    /// Load a software signer giving it a name
    LoadSoftware {
        #[arg(long)]
        name: String,

        #[arg(long)]
        mnemonic: String, // TODO is it right to have the mnemonic as arg?
    },

    /// Load a Jade signer giving it a name
    LoadJade {
        #[arg(long)]
        name: String,

        /// Identifier of the jade (20 bytes as 40 hex chars)
        #[arg(long)]
        id: String,

        /// The socket address to connect to jade emulator
        #[arg(long)]
        emulator: Option<SocketAddr>,
    },

    /// Load a signer (software, serial, external) giving it a name
    LoadExternal {
        #[arg(long)]
        name: String,

        #[arg(long)]
        fingerprint: String,
    },

    /// Unload a software signer
    Unload {
        #[arg(long)]
        name: String,
    },

    /// List loaded signers
    List,

    /// Sign a transaction
    Sign {
        #[arg(long)]
        name: String,

        pset: String,
    },

    ///  Prints a singlesig descriptor using this signer key
    SinglesigDesc {
        #[arg(long)]
        name: String,

        #[arg(long)]
        descriptor_blinding_key: BlindingKeyKind,

        #[arg(long)]
        kind: SinglesigKind,
    },

    /// Get an extended public key from the signer
    Xpub {
        #[arg(long)]
        name: String,

        #[arg(long)]
        kind: XpubKind,
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

#[derive(ValueEnum, Clone, Debug)]
pub enum MultisigKind {
    Wsh,
}

impl Display for MultisigKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultisigKind::Wsh => write!(f, "wsh"),
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

        /// Replace asset ids with tickers when possible
        #[arg(long, action)]
        with_tickers: bool,
    },

    /// Create an unsigned transaction (PSET)
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

    /// Issue an asset
    Issue {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        /// The number of units of the newly issued asset
        #[arg(long)]
        satoshi_asset: u64,

        /// Address receiving the newly issued asset.
        /// If not specified an external address of the wallet identified by `name` will be used
        #[arg(long)]
        address_asset: Option<String>,

        /// Number of reissuance token emitted, common choice are 0 or 1
        #[arg(long)]
        satoshi_token: u64,

        /// Address receiving the reissuance token(s).
        /// Must be specified is satoshi_token is greater than 0, otherwise could be
        #[arg(long)]
        address_token: Option<String>,

        /// Specify the JSON contract as string, you can use the included util to generate it
        #[arg(long)]
        contract: Option<String>,

        // TODO default value
        /// To optionally specify a fee
        #[arg(long)]
        fee_rate: Option<f32>,
    },

    /// Prints a list of issuances made by this wallet
    Issuances {},

    /// Reissue a previously issued asset, needs ownership of the issuance token
    Reissue {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        /// The asset to re-issue
        #[arg(long)]
        asset: String,

        /// The number of units of the re-issued asset
        #[arg(long)]
        satoshi_asset: u64,

        /// Address receiving the re-issued asset.
        /// If not specified an external address of the wallet identified by `name` will be used
        #[arg(long)]
        address_asset: Option<String>,

        // TODO default value
        /// To optionally specify a fee
        #[arg(long)]
        fee_rate: Option<f32>,
    },

    /// Print a multisig descriptor
    MultisigDesc {
        #[arg(long)]
        descriptor_blinding_key: BlindingKeyKind,

        #[arg(long)]
        kind: MultisigKind,

        #[arg(long)]
        threshold: u32,

        #[arg(long, required = true)]
        keyorigin_xpub: Vec<String>,
    },

    /// Try to finalize the PSET and broadcast the transaction
    Broadcast {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        /// Do the finalization without the broadcast
        #[arg(long)]
        dry_run: bool,

        pset: String,
    },

    /// Get detailed information about the wallet
    Details {
        /// Wallet name
        #[arg(short, long)]
        name: String,
    },

    Combine {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        /// PSETs to combine
        ///
        /// Can be specified multiple times.
        #[arg(short, long, required = true)]
        pset: Vec<String>,
    },

    PsetDetails {
        /// Wallet name
        #[arg(short, long)]
        name: String,

        /// PSET
        #[arg(short, long, required = true)]
        pset: String,

        /// Replace asset ids with tickers when possible
        #[arg(long, action)]
        with_tickers: bool,
    },
}

#[derive(Debug, Args)]
pub struct AssetArgs {
    #[command(subcommand)]
    pub command: AssetCommand,
}

#[derive(Debug, Subcommand)]
pub enum AssetCommand {
    /// Helper to create a valid JSON contract
    Contract {
        /// Http domain of the issuer
        #[arg(long)]
        domain: String,

        /// Http domain of the issuer
        #[arg(long)]
        issuer_pubkey: String,

        /// Name of the asset
        #[arg(long)]
        name: String,

        /// Precision of the asset, as in number of digits to represent fractional part.
        #[arg(long, default_value = "0")]
        precision: u8,

        /// Ticker of the asset
        #[arg(long)]
        ticker: String,

        /// Version
        // TODO since now only 0 exists, should we default to 0 internally without giving the option to override?
        #[arg(long, default_value = "0")]
        version: u8,
    },

    /// Get detailed information about an asset
    Details {
        /// Asset ID in hex
        #[arg(short, long)]
        asset: String,
    },

    /// List assets
    List,

    /// Insert an asset
    Insert {
        /// Asset ID in hex
        #[arg(short, long)]
        asset: String,

        /// The JSON contract
        ///
        /// You can fetch it from the asset registry from
        /// `https://assets.blockstream.info/<ASSET-ID-HEX>`
        /// alongside the issuenace prevout.
        #[arg(long)]
        contract: String,

        /// Issuance prevout txid
        #[arg(long)]
        prev_txid: String,

        /// Issuance prevout vout
        #[arg(long)]
        prev_vout: u32,

        /// Whether the issuance was blinded or not
        #[arg(long, default_value_t = false)]
        is_confidential: bool,
    },

    /// Remove an asset
    Remove {
        /// Asset ID in hex
        #[arg(short, long)]
        asset: String,
    },
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
