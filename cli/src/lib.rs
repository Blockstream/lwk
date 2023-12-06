#![doc = include_str!("../README.md")]

use std::{sync::mpsc::RecvTimeoutError, time::Duration};

use anyhow::{anyhow, Context};
use app::Config;
use clap::CommandFactory;
use serde_json::Value;
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

use crate::args::{AssetCommand, CliCommand, Network, ServerCommand, SignerCommand, WalletCommand};
pub use args::Cli;

pub use args::{AssetSubCommandsEnum, SignerSubCommandsEnum, WalletSubCommandsEnum};

mod args;
mod schema;

pub fn inner_main(args: args::Cli) -> anyhow::Result<Value> {
    let directive = if let CliCommand::Server(args::ServerArgs {
        command: ServerCommand::Start,
    }) = args.command
    {
        LevelFilter::INFO.into()
    } else {
        LevelFilter::WARN.into()
    };

    let (appender, _guard) = tracing_appender::non_blocking(std::io::stderr());
    let filter = EnvFilter::builder()
        .with_default_directive(directive)
        .from_env_lossy();
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_writer(appender)
        .finish();
    match tracing::subscriber::set_global_default(subscriber) {
        Ok(_) => tracing::info!("logging initialized"),
        Err(_) => tracing::debug!("logging already initialized"),
    }

    tracing::info!("CLI initialized with args: {:?}", args);

    // start the app with default host/port
    let mut config = match args.network {
        Network::Mainnet => Config::default_mainnet(),
        Network::Testnet => Config::default_testnet(),
        Network::Regtest => Config::default_regtest(
            &args
                .electrum_url
                .ok_or_else(|| anyhow!("on regtest you have to specify --electrum-url"))?,
        ),
    };
    config.datadir = args.datadir.unwrap_or_else(Config::default_home);
    config.addr = args.addr;

    let mut app = app::App::new(config)?;
    // get a client to make requests
    let client = app.client()?;

    // verify the server is up
    if let CliCommand::Server(args::ServerArgs {
        command: ServerCommand::Start,
    })
    | CliCommand::GenerateCompletion { .. } = args.command
    {
        // unless I am starting it
    } else if client.version().is_err() {
        return Err(anyhow!("Is the server at {:?} running?", app.addr()));
    }

    Ok(match args.command {
        CliCommand::Server(a) => {
            match a.command {
                ServerCommand::Start => {
                    let (tx, rx) = std::sync::mpsc::channel();
                    let set_handler_result = ctrlc::try_set_handler(move || {
                        tx.send(()).expect("Could not send signal on channel.")
                    });

                    app.run().with_context(|| {
                        format!(
                            "Cannot start the server at \"{}\". It is probably already running.",
                            app.addr()
                        )
                    })?;

                    // get the app version
                    let version = client.version()?.version;
                    tracing::info!("App running version {}", version);

                    if set_handler_result.is_ok() {
                        loop {
                            match rx.recv_timeout(Duration::from_millis(100)) {
                                Ok(_) => {
                                    tracing::debug!("Stopping");
                                    app.stop()?;
                                }
                                Err(e) => match e {
                                    RecvTimeoutError::Timeout => {
                                        if app.is_running().unwrap_or(false) {
                                            continue;
                                        } else {
                                            break;
                                        }
                                    }
                                    RecvTimeoutError::Disconnected => {
                                        return Err(anyhow!("RecvTimeoutError::Disconnected"))
                                    }
                                },
                            }
                        }
                    }
                    app.join_threads()?;
                    tracing::info!("Threads ended");
                }
                ServerCommand::Stop => {
                    client.stop()?;
                }
            }

            Value::Null
        }
        CliCommand::Signer(a) => match a.command {
            SignerCommand::Generate => {
                let j = client.generate_signer()?;
                serde_json::to_value(j)?
            }
            SignerCommand::JadeId { emulator } => {
                let j = client.signer_jade_id(emulator)?;
                serde_json::to_value(j)?
            }
            SignerCommand::Sign { name, pset } => {
                let r = client.sign(name, pset)?;
                serde_json::to_value(r)?
            }
            SignerCommand::LoadSoftware { name, mnemonic } => {
                let j = client.signer_load_software(name, mnemonic)?;
                serde_json::to_value(j)?
            }
            SignerCommand::LoadJade { name, id, emulator } => {
                let j = client.signer_load_jade(name, id, emulator)?;
                serde_json::to_value(j)?
            }
            SignerCommand::LoadExternal { name, fingerprint } => {
                let j = client.signer_load_external(name, fingerprint)?;
                serde_json::to_value(j)?
            }
            SignerCommand::List => serde_json::to_value(client.list_signers()?)?,
            SignerCommand::Unload { name } => {
                let r = client.unload_signer(name)?;
                serde_json::to_value(r)?
            }
            SignerCommand::SinglesigDesc {
                name,
                descriptor_blinding_key,
                kind,
            } => {
                let r = client.singlesig_descriptor(
                    name,
                    descriptor_blinding_key.to_string(),
                    kind.to_string(),
                )?;
                serde_json::to_value(r)?
            }
            SignerCommand::Xpub { name, kind } => {
                let r = client.xpub(name, kind.to_string())?;
                serde_json::to_value(r)?
            }
        },
        CliCommand::Wallet(a) => match a.command {
            WalletCommand::Load { descriptor, name } => {
                let r = client.load_wallet(descriptor, name)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Unload { name } => {
                let r = client.unload_wallet(name)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Balance { name, with_tickers } => {
                let r = client.balance(name, with_tickers)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Send {
                name,
                recipient,
                fee_rate,
            } => {
                let mut addressees = vec![];
                for rec in recipient {
                    addressees.push(rec.try_into()?);
                }

                let r = client.send_many(name, addressees, fee_rate)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Address { index, name } => {
                let r = client.address(name, index)?;
                serde_json::to_value(r)?
            }
            WalletCommand::List => serde_json::to_value(client.list_wallets()?)?,
            WalletCommand::Issue {
                name,
                satoshi_asset,
                address_asset,
                satoshi_token,
                address_token,
                contract,
                fee_rate,
            } => {
                let r = client.issue(
                    name,
                    satoshi_asset,
                    address_asset,
                    satoshi_token,
                    address_token,
                    contract,
                    fee_rate,
                )?;
                serde_json::to_value(r)?
            }
            WalletCommand::Reissue {
                name,
                asset,
                satoshi_asset,
                address_asset,
                fee_rate,
            } => {
                let r = client.reissue(name, asset, satoshi_asset, address_asset, fee_rate)?;
                serde_json::to_value(r)?
            }
            WalletCommand::MultisigDesc {
                descriptor_blinding_key,
                kind,
                threshold,
                keyorigin_xpub,
            } => {
                let r = client.multisig_descriptor(
                    descriptor_blinding_key.to_string(),
                    kind.to_string(),
                    threshold,
                    keyorigin_xpub,
                )?;
                serde_json::to_value(r)?
            }
            WalletCommand::Broadcast {
                dry_run,
                pset,
                name,
            } => {
                let r = client.broadcast(name, dry_run, pset)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Details { name } => {
                let r = client.wallet_details(name)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Combine { name, pset } => {
                let r = client.wallet_combine(name, pset)?;
                serde_json::to_value(r)?
            }
            WalletCommand::PsetDetails {
                name,
                pset,
                with_tickers,
            } => {
                let r = client.wallet_pset_details(name, pset, with_tickers)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Utxos { name } => {
                let r = client.wallet_utxos(name)?;
                serde_json::to_value(r)?
            }
        },
        CliCommand::Asset(a) => match a.command {
            AssetCommand::Contract {
                domain,
                issuer_pubkey,
                name,
                precision,
                ticker,
                version,
            } => {
                let r = client.contract(domain, issuer_pubkey, name, precision, ticker, version)?;
                serde_json::to_value(r)?
            }
            AssetCommand::Details { asset } => {
                let r = client.asset_details(asset)?;
                serde_json::to_value(r)?
            }
            AssetCommand::List => serde_json::to_value(client.list_assets()?)?,
            AssetCommand::Insert {
                asset,
                contract,
                prev_txid,
                prev_vout,
                is_confidential,
            } => {
                let r = client.asset_insert(
                    asset,
                    contract,
                    prev_txid,
                    prev_vout,
                    Some(is_confidential),
                )?;
                serde_json::to_value(r)?
            }
            AssetCommand::Remove { asset } => {
                let r = client.asset_remove(asset)?;
                serde_json::to_value(r)?
            }
        },
        CliCommand::Schema(a) => schema::schema(a, client)?,
        CliCommand::GenerateCompletion { shell } => {
            let mut result = vec![];
            clap_complete::generate(shell, &mut Cli::command(), "cli", &mut result);
            let s = String::from_utf8(result)?;
            Value::String(s)
        }
    })
}
