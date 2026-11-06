#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

use anyhow::{anyhow, Context};
use args::LiquidexCommand;
use clap::CommandFactory;
use env_logger::Env;
use lwk_app::{Client, Config};
use serde_json::Value;

use crate::args::{
    Amp2Command, AssetCommand, CliCommand, Network, ServerCommand, ServerStartArgs, SignerCommand,
    WalletCommand,
};
pub use args::Cli;

pub use args::{
    AssetSubCommandsEnum, ServerSubCommandsEnum, SignerSubCommandsEnum, WalletSubCommandsEnum,
};

mod args;
mod schema;

pub fn inner_main(args: args::Cli) -> anyhow::Result<Value> {
    let directive = if let CliCommand::Server(args::ServerArgs {
        command: ServerCommand::Start { .. },
    }) = args.command
    {
        "info"
    } else {
        "warn"
    };
    let mut builder = env_logger::Builder::from_env(Env::default().default_filter_or(directive));

    match builder.try_init() {
        Ok(_) => log::info!("logging initialized"),
        Err(_) => log::debug!("logging already initialized"),
    }

    log::info!("CLI initialized with args: {args:?}");

    // TODO: improve network types conversion or comparison
    let (network, default_port) = match args.network {
        Network::Mainnet => ("liquid", 32110),
        Network::Testnet => ("liquid-testnet", 32111),
        Network::Regtest => ("liquid-regtest", 32112),
    };

    let addr = args
        .addr
        .unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), default_port));

    // Every command needs the datadir to find the RPC auth cookie the server wrote there, not
    // just `server start`.
    let datadir = args
        .datadir
        .clone()
        .unwrap_or_else(|| Config::default_home().unwrap_or(std::path::PathBuf::from(".")));
    let cookie_path = datadir.join(network).join(".cookie");
    let client = lwk_app::Client::new_with_cookie(addr, Some(cookie_path))?;

    // verify the server is up if needed
    if args.command.requires_server_running() {
        let version = client
            .version()
            .with_context(|| format!("Is the server at {addr:?} running?"))?;
        let server_network = version.network;

        if server_network != network {
            return Err(anyhow!(
                "Inconsistent networks (cli: {network}, server: {server_network})",
            ));
        }
    }

    Ok(match args.command {
        CliCommand::Server(a) => server(&client, a.command, args.network, addr, datadir)?,
        CliCommand::Signer(a) => signer(&client, a.command)?,
        CliCommand::Wallet(a) => wallet(&client, a.command)?,
        CliCommand::Liquidex(a) => liquidex(&client, a.command)?,
        CliCommand::Asset(a) => asset(&client, a.command)?,
        CliCommand::Amp2(a) => amp2(&client, a.command)?,
        CliCommand::Schema(a) => schema::schema(a, client)?,
        CliCommand::GenerateCompletion { shell } => {
            let mut result = vec![];
            clap_complete::generate(shell, &mut Cli::command(), "lwk_cli", &mut result);
            let s = String::from_utf8(result)?;
            Value::String(s)
        }

        #[cfg(feature = "bindings")]
        CliCommand::Generate { .. } => {
            uniffi::uniffi_bindgen_main();
            Value::Null
        }
    })
}

fn server(
    client: &Client,
    cmd: ServerCommand,
    network: Network,
    addr: SocketAddr,
    datadir: PathBuf,
) -> anyhow::Result<Value> {
    match cmd {
        ServerCommand::Start(opts) => run_server_start(client, network, addr, datadir, opts)?,
        ServerCommand::Scan => {
            client.scan()?;
        }
        ServerCommand::Stop => {
            client.stop()?;
        }
    }
    Ok(Value::Null)
}

fn build_config(
    network: Network,
    addr: SocketAddr,
    datadir: PathBuf,
    opts: ServerStartArgs,
) -> anyhow::Result<Config> {
    let mut config = match network {
        Network::Mainnet => Config::default_mainnet(datadir),
        Network::Testnet => Config::default_testnet(datadir),
        Network::Regtest => Config::default_regtest(datadir),
    };
    if let Some(timeout) = opts.timeout {
        config.timeout = Duration::from_secs(timeout);
    }
    if let Some(scanning_interval) = opts.scanning_interval {
        config.scanning_interval = Duration::from_secs(scanning_interval);
    }
    if let Some(url) = opts.server_url {
        config.server_url = url;
    } else if let Network::Regtest = network {
        anyhow::bail!("on regtest you have to specify --electrum-url");
    }
    config.server_type = opts.server_type.to_string();

    if let Some(url) = opts.registry_url {
        config.registry_url = url;
    }
    if let Some(amp2_url) = opts.amp2_url {
        config.amp2_url = amp2_url;
    }
    if let Some(amp2_keyorigin_xpub) = opts.amp2_keyorigin_xpub {
        config.amp2_keyorigin_xpub = amp2_keyorigin_xpub;
    }

    config.addr = addr;
    config.with_experimental_blinders = opts.with_experimental_blinders;
    config.allow_plaintext_with_token = opts.auth.auth_allow_plaintext_with_token;
    config.token_provider = opts.auth.token_provider()?;
    Ok(config)
}

fn run_server_start(
    client: &Client,
    network: Network,
    addr: SocketAddr,
    datadir: PathBuf,
    opts: ServerStartArgs,
) -> anyhow::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = ctrlc::try_set_handler(move || {
        // The receiver is gone only if the server is already shutting down
        let _ = tx.send(());
    });

    let config = build_config(network, addr, datadir, opts)?;
    let mut app = lwk_app::App::new(config)?;
    app.run()?;

    // get the app version
    let version = client.version()?.version;
    log::info!("App running version {version}");

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(_) => {
                log::debug!("Received ctrl-c signal");
                break;
            }
            Err(_) => {
                if app.is_running().unwrap_or(false) {
                    continue;
                } else {
                    log::debug!("Received stop signal");
                    break;
                }
            }
        }
    }
    app.stop()?;
    app.join_threads()?;
    log::info!("Threads ended");
    Ok(())
}

fn signer(client: &Client, cmd: SignerCommand) -> anyhow::Result<Value> {
    Ok(match cmd {
        SignerCommand::Generate => to_json(client.signer_generate())?,
        SignerCommand::JadeId { emulator } => to_json(client.signer_jade_id(emulator))?,
        SignerCommand::Sign {
            signer,
            pset,
            allow_sighash_single_anyonecanpay,
            dangerous_allow_any_sighash,
        } => to_json(client.signer_sign(
            signer,
            pset,
            allow_sighash_single_anyonecanpay,
            dangerous_allow_any_sighash,
        ))?,
        SignerCommand::LoadSoftware {
            signer,
            mnemonic,
            persist,
        } => {
            let persist = persist.ok_or_else(|| anyhow!("--persist is required"))?;
            to_json(client.signer_load_software(signer, mnemonic, persist))?
        }
        SignerCommand::LoadJade {
            signer,
            id,
            emulator,
        } => to_json(client.signer_load_jade(signer, id, emulator))?,
        SignerCommand::LoadExternal {
            signer,
            fingerprint,
        } => to_json(client.signer_load_external(signer, fingerprint))?,
        SignerCommand::List => to_json(client.signer_list())?,
        SignerCommand::Details { signer } => to_json(client.signer_details(signer))?,
        SignerCommand::Unload { signer } => to_json(client.signer_unload(signer))?,
        SignerCommand::SinglesigDesc {
            signer,
            descriptor_blinding_key,
            kind,
        } => to_json(client.signer_singlesig_descriptor(
            signer,
            descriptor_blinding_key.to_string(),
            kind.to_string(),
        ))?,
        SignerCommand::Xpub { signer, kind } => {
            to_json(client.signer_xpub(signer, kind.to_string()))?
        }
        SignerCommand::DeriveBip85 {
            signer,
            index,
            word_count,
        } => to_json(client.signer_derive_bip85(signer, index, word_count))?,
        SignerCommand::RegisterMultisig { signer, wallet } => {
            to_json(client.signer_register_multisig(signer, wallet))?
        }
    })
}

fn wallet(client: &Client, cmd: WalletCommand) -> anyhow::Result<Value> {
    Ok(match cmd {
        WalletCommand::Load { descriptor, wallet } => {
            to_json(client.wallet_load(descriptor, wallet))?
        }
        WalletCommand::Unload { wallet } => to_json(client.wallet_unload(wallet))?,
        WalletCommand::Balance {
            wallet,
            with_tickers,
        } => to_json(client.wallet_balance(wallet, with_tickers))?,
        WalletCommand::Send {
            wallet,
            recipient,
            fee_rate,
        } => {
            let mut addressees = vec![];
            for rec in recipient {
                addressees.push(
                    rec.parse()
                        .with_context(|| "error parsing recipient argument")?,
                );
            }

            to_json(client.wallet_send_many(wallet, addressees, fee_rate))?
        }
        WalletCommand::Drain {
            wallet,
            address,
            fee_rate,
        } => to_json(client.wallet_drain(wallet, address, fee_rate))?,
        WalletCommand::Address {
            index,
            wallet,
            signer,
            with_text_qr,
            with_uri_qr,
        } => to_json(client.wallet_address(wallet, index, signer, with_text_qr, with_uri_qr))?,
        WalletCommand::List => to_json(client.wallet_list())?,
        WalletCommand::Issue {
            wallet,
            satoshi_asset,
            address_asset,
            satoshi_token,
            address_token,
            contract,
            fee_rate,
        } => to_json(client.wallet_issue(
            wallet,
            satoshi_asset,
            address_asset,
            satoshi_token,
            address_token,
            contract,
            fee_rate,
        ))?,
        WalletCommand::Reissue {
            wallet,
            asset,
            satoshi_asset,
            address_asset,
            fee_rate,
        } => to_json(client.wallet_reissue(wallet, asset, satoshi_asset, address_asset, fee_rate))?,
        WalletCommand::Burn {
            wallet,
            asset,
            satoshi_asset,
            fee_rate,
        } => to_json(client.wallet_burn(wallet, asset, satoshi_asset, fee_rate))?,
        WalletCommand::MultisigDesc {
            descriptor_blinding_key,
            kind,
            threshold,
            keyorigin_xpub,
        } => to_json(client.wallet_multisig_descriptor(
            descriptor_blinding_key.to_string(),
            kind.to_string(),
            threshold,
            keyorigin_xpub,
        ))?,
        WalletCommand::Broadcast {
            dry_run,
            pset,
            wallet,
        } => to_json(client.wallet_broadcast(wallet, dry_run, pset))?,
        WalletCommand::Details { wallet } => to_json(client.wallet_details(wallet))?,
        WalletCommand::Combine { wallet, pset } => to_json(client.wallet_combine(wallet, pset))?,
        WalletCommand::PsetDetails {
            wallet,
            pset,
            with_tickers,
        } => to_json(client.wallet_pset_details(wallet, pset, with_tickers))?,
        WalletCommand::Utxos { wallet } => to_json(client.wallet_utxos(wallet))?,
        WalletCommand::Txs {
            wallet,
            with_tickers,
        } => to_json(client.wallet_txs(wallet, with_tickers))?,
        WalletCommand::Tx {
            wallet,
            txid,
            fetch,
        } => to_json(client.wallet_tx(wallet, txid, fetch))?,
        WalletCommand::TxDetails { wallet, txid } => {
            to_json(client.wallet_tx_details(wallet, txid))?
        }
        WalletCommand::SetTxMemo { wallet, txid, memo } => {
            to_json(client.wallet_set_tx_memo(wallet, txid, memo))?
        }
        WalletCommand::SetAddrMemo {
            wallet,
            address,
            memo,
        } => to_json(client.wallet_set_addr_memo(wallet, address, memo))?,
        WalletCommand::DumpUnblinded { wallet, txid } => {
            to_json(client.wallet_dump_unblinded(wallet, txid))?
        }
    })
}

fn liquidex(client: &Client, cmd: LiquidexCommand) -> anyhow::Result<Value> {
    Ok(match cmd {
        LiquidexCommand::Make {
            wallet,
            txid,
            vout,
            asset,
            satoshi,
        } => to_json(client.liquidex_make(wallet, txid, vout, asset, satoshi))?,
        LiquidexCommand::Take { wallet, proposal } => {
            to_json(client.liquidex_take(wallet, proposal))?
        }
        LiquidexCommand::ToProposal { pset } => to_json(client.liquidex_to_proposal(pset))?,
    })
}

fn asset(client: &Client, cmd: AssetCommand) -> anyhow::Result<Value> {
    Ok(match cmd {
        AssetCommand::Contract {
            domain,
            issuer_pubkey,
            name,
            precision,
            ticker,
            version,
        } => {
            to_json(client.asset_contract(domain, issuer_pubkey, name, precision, ticker, version))?
        }
        AssetCommand::Details { asset } => to_json(client.asset_details(asset))?,
        AssetCommand::List => to_json(client.asset_list())?,
        AssetCommand::Insert {
            asset,
            issuance_tx,
            contract,
        } => to_json(client.asset_insert(asset, issuance_tx, contract))?,
        AssetCommand::Remove { asset } => to_json(client.asset_remove(asset))?,
        AssetCommand::FromRegistry { asset } => to_json(client.asset_from_registry(asset))?,
        AssetCommand::Publish { asset } => to_json(client.asset_publish(asset))?,
    })
}

fn amp2(client: &Client, cmd: Amp2Command) -> anyhow::Result<Value> {
    Ok(match cmd {
        Amp2Command::Descriptor {
            signer,
            descriptor_blinding_key,
        } => to_json(client.amp2_descriptor(signer, descriptor_blinding_key))?,
        Amp2Command::Register {
            signer,
            descriptor_blinding_key,
        } => to_json(client.amp2_register(signer, descriptor_blinding_key))?,
        Amp2Command::Cosign { pset } => to_json(client.amp2_cosign(pset))?,
    })
}

/// Convert a successful result into a JSON [`Value`], propagating errors.
fn to_json<T: serde::Serialize, E: Into<anyhow::Error>>(r: Result<T, E>) -> anyhow::Result<Value> {
    Ok(serde_json::to_value(r.map_err(Into::into)?)?)
}
