use std::{fmt::Display, net::SocketAddr, path::PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(ValueEnum, Clone, Debug)]
pub enum Network {
    Mainnet,
    Testnet,
    Regtest,
}

/// A liquid wallet with watch-only confidential descriptors and hardware signers.
/// WARNING: not yet for production use, expect bugs, breaking changes and loss of funds.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Network
    #[structopt(short, long, default_value = "testnet", env)]
    pub network: Network,

    /// Server socket address
    #[arg(long, env)]
    pub addr: Option<SocketAddr>,

    /// The sub command
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Server commands
    Server(ServerArgs),

    /// Signer commands
    Signer(SignerArgs),

    /// Wallet commands
    Wallet(WalletArgs),

    /// Asset commands
    Asset(AssetArgs),

    /// Print JSON schema of RPC requests and responses
    ///
    /// E.g. `lwk_cli schema response wallet details` returns the response parameters for
    /// `lwk_cli wallet details`
    Schema(SchemaArgs),

    #[clap(hide = true)]
    GenerateCompletion { shell: Shell },

    /// Generate bindings, this is here so that we have a unique binary across the workspace.
    /// The fields are just a copy of what you need in [`uniffi::uniffi_bindgen_main()`] so that
    /// this subcommand is compatible with that. To use any other option available there it must be
    /// elencated also here
    #[clap(hide = true)]
    #[cfg(feature = "bindings")]
    Generate {
        #[arg(long)]
        library: String,
        #[arg(long)]
        language: String,
        #[arg(long)]
        out_dir: String,
    },
}

#[allow(dead_code)] // not sure why it's needed but without there is a warning even if the fn is called
impl CliCommand {
    #[cfg(not(feature = "bindings"))]
    pub(crate) fn requires_server_running(&self) -> bool {
        !matches!(
            self,
            CliCommand::Server(crate::args::ServerArgs {
                command: ServerCommand::Start { .. },
            }) | CliCommand::GenerateCompletion { .. }
        )
    }

    #[cfg(feature = "bindings")]
    pub(crate) fn requires_server_running(&self) -> bool {
        !matches!(
            self,
            CliCommand::Server(crate::args::ServerArgs {
                command: ServerCommand::Start { .. },
            }) | CliCommand::GenerateCompletion { .. }
                | CliCommand::Generate { .. }
        )
    }
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
    Server(ServerSubCommands),
    Wallet(WalletSubCommands),
    Signer(SignerSubCommands),
    Asset(AssetSubCommands),
    Schema,
}

#[derive(Debug, Args)]
pub struct ServerSubCommands {
    #[command(subcommand)]
    pub command: ServerSubCommandsEnum,
}

#[derive(Debug, Subcommand, ValueEnum, Clone)]
pub enum ServerSubCommandsEnum {
    // Start is a special command
    Scan,
    Stop,
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
    Reissue,
    MultisigDesc,
    Broadcast,
    Details,
    Combine,
    PsetDetails,
    Utxos,
    Txs,
    SetTxMemo,
    SetAddrMemo,
}

#[derive(Debug, Args)]
pub struct SignerSubCommands {
    #[command(subcommand)]
    pub command: SignerSubCommandsEnum,
}

#[derive(Debug, Subcommand, ValueEnum, Clone)]
pub enum SignerSubCommandsEnum {
    Generate,
    JadeId,
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
    Publish,
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
    Bip49,
    Bip87,
}

impl std::fmt::Display for XpubKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            XpubKind::Bip84 => write!(f, "bip84"),
            XpubKind::Bip49 => write!(f, "bip49"),
            XpubKind::Bip87 => write!(f, "bip87"),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum SignerCommand {
    /// Generate a software signer, returns a mnemonic
    Generate,

    /// Probe connected Jades, unlocks and returns identifiers that allows to load a Jade
    JadeId {
        /// The socket address to connect to jade emulator
        #[arg(long)]
        emulator: Option<SocketAddr>,
    },

    /// Load a software signer giving it a name
    LoadSoftware {
        #[arg(short, long, env)]
        signer: String,

        #[arg(long)]
        mnemonic: String, // TODO is it right to have the mnemonic as arg?
    },

    /// Load a Jade signer giving it a name
    LoadJade {
        #[arg(short, long, env)]
        signer: String,

        /// Identifier of the jade (20 bytes as 40 hex chars)
        #[arg(long)]
        id: String,

        /// The socket address to connect to jade emulator
        #[arg(long)]
        emulator: Option<SocketAddr>,
    },

    /// Load a signer (software, serial, external) giving it a name
    LoadExternal {
        #[arg(short, long, env)]
        signer: String,

        #[arg(long)]
        fingerprint: String,
    },

    /// Unload a software signer
    Unload {
        #[arg(short, long, env)]
        signer: String,
    },

    /// List loaded signers
    List,

    /// Sign a transaction
    Sign {
        #[arg(short, long, env)]
        signer: String,

        #[arg(long)]
        pset: String,
    },

    ///  Prints a singlesig descriptor using this signer key
    SinglesigDesc {
        #[arg(short, long, env)]
        signer: String,

        #[arg(long)]
        descriptor_blinding_key: BlindingKeyKind,

        #[arg(long)]
        kind: SinglesigKind,
    },

    /// Get an extended public key from the signer
    Xpub {
        #[arg(short, long, env)]
        signer: String,

        #[arg(long)]
        kind: XpubKind,
    },

    /// Register a multisig wallet
    ///
    /// This is needed to correctly display change outputs Jade.
    /// For other signers this command does nothing.
    RegisterMultisig {
        /// Signer name
        #[arg(short, long, env)]
        signer: String,

        /// Wallet name
        #[arg(long)]
        wallet: String,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum BlindingKeyKind {
    Slip77,
    Slip77Rand,
    Elip151,
}

impl Display for BlindingKeyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlindingKeyKind::Slip77 => write!(f, "slip77"),
            BlindingKeyKind::Slip77Rand => write!(f, "slip77-rand"),
            BlindingKeyKind::Elip151 => write!(f, "elip151"),
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
        #[arg(short, long, env)]
        wallet: String,

        #[arg(short, long)]
        descriptor: String,
    },

    /// Unload a wallet
    Unload {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,
    },

    /// List existing loaded wallets
    List,

    /// Get an address from the given wallet name
    Address {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

        #[arg(long)]
        index: Option<u32>,

        /// Signer name
        ///
        /// Display the address on hardware signers.
        #[arg(short, long, env)]
        signer: Option<String>,
    },

    /// Get the balance of the given wallet name
    Balance {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

        /// Replace asset ids with tickers when possible
        #[arg(long, action)]
        with_tickers: bool,
    },

    /// Create an unsigned transaction (PSET)
    Send {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

        /// Specify a recipient in the form "address:satoshi:asset_id"
        ///
        /// Can be specified multiple times.
        ///
        /// Address can either be a valid address or "burn" if you want to burn the asset, i.e.
        /// create a provably unspendable output.
        #[arg(long, required = true)]
        recipient: Vec<String>,

        /// Fee rate to use
        #[arg(long)]
        fee_rate: Option<f32>,
    },

    /// Issue an asset
    Issue {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

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

    /// Reissue a previously issued asset, needs ownership of the issuance token
    Reissue {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

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
        #[arg(short, long, env)]
        wallet: String,

        /// Do the finalization without the broadcast
        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        pset: String,
    },

    /// Get detailed information about the wallet
    Details {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,
    },

    /// Combine PSETs
    Combine {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

        /// PSETs to combine
        ///
        /// Can be specified multiple times.
        #[arg(short, long, required = true)]
        pset: Vec<String>,
    },

    /// Get the details of a PSET
    PsetDetails {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

        /// PSET
        #[arg(short, long, required = true)]
        pset: String,

        /// Replace asset ids with tickers when possible
        #[arg(long, action)]
        with_tickers: bool,
    },

    /// Get the wallet unspent transaction outputs
    Utxos {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,
    },

    /// Get the wallet transactions
    Txs {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

        /// Replace asset ids with tickers when possible
        #[arg(long, action)]
        with_tickers: bool,
    },

    /// Set a wallet tx memo
    SetTxMemo {
        /// Wallet name
        #[arg(short, long, env)]
        wallet: String,

        /// The transaction id
        #[arg(long)]
        txid: String,

        /// The memo to set
        #[arg(long)]
        memo: String,
    },

    /// Set a wallet address memo
    SetAddrMemo {
        /// Wallet name
        #[arg(short, long)]
        wallet: String,

        /// The address
        #[arg(long)]
        address: String,

        /// The memo to set
        #[arg(long)]
        memo: String,
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

        /// A pubkey (66 hex chars) owned by the issuer to handle asset metadata updates
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
        /// alongside the issuance prevout.
        #[arg(long)]
        contract: String,

        /// The issuance transaction in hex
        ///
        /// You can fetch it from your node or from a block explorer,
        /// e.g. `https://blockstream.info/liquid/api/<TXID>/hex`
        #[arg(long)]
        issuance_tx: String,
    },

    /// Remove an asset
    Remove {
        /// Asset ID in hex
        #[arg(short, long)]
        asset: String,
    },

    /// Try to publish the contract identified by the given asset id
    ///
    /// The asset must be stored in the server so that the contract can be fetched internally
    ///
    /// It may fail if there isn't a proof on the issuer's domain, if failing it gives info on how
    /// to do this.
    Publish {
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
    Start {
        /// Electrum URL, if not specified a reasonable default is used according to the network
        #[arg(short, long)]
        electrum_url: Option<String>,

        /// Location for logs, server state, and other LWK data
        ///
        /// Default is `$HOME/.lwk`, or `./.lwk` if unable to determine the home dir
        #[arg(long)]
        datadir: Option<PathBuf>,

        /// Timeout for RPC and HWW requests (seconds)
        #[arg(long)]
        timeout: Option<u64>,

        /// Interval between blockchain scans (seconds)
        #[arg(long)]
        scanning_interval: Option<u64>,

        /// Ignore start errors
        #[arg(long)]
        ignore_start_error: Option<bool>,
    },

    /// Wait until an entire blockchain scan has been completed
    Scan,

    /// Stop the server
    ///
    /// Alternatively the server can be stopped also with SIGINT (ctrl-c)
    Stop,
}
