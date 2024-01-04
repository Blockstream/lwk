#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

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
    let datadir = args
        .datadir
        .unwrap_or_else(|| Config::default_home().unwrap_or(std::path::PathBuf::from(".")));
    let mut config = match args.network {
        Network::Mainnet => Config::default_mainnet(datadir),
        Network::Testnet => Config::default_testnet(datadir),
        Network::Regtest => Config::default_regtest(
            &args
                .electrum_url
                .ok_or_else(|| anyhow!("on regtest you have to specify --electrum-url"))?,
            datadir,
        ),
    };
    config.addr = args.addr;

    let mut app = app::App::new(config)?;
    // get a client to make requests
    let client = app.client()?;

    // verify the server is up if needed
    if args.command.requires_server_running() && client.version().is_err() {
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
            SignerCommand::Sign { signer, pset } => {
                let r = client.sign(signer, pset)?;
                serde_json::to_value(r)?
            }
            SignerCommand::LoadSoftware { signer, mnemonic } => {
                let j = client.signer_load_software(signer, mnemonic)?;
                serde_json::to_value(j)?
            }
            SignerCommand::LoadJade {
                signer,
                id,
                emulator,
            } => {
                let j = client.signer_load_jade(signer, id, emulator)?;
                serde_json::to_value(j)?
            }
            SignerCommand::LoadExternal {
                signer,
                fingerprint,
            } => {
                let j = client.signer_load_external(signer, fingerprint)?;
                serde_json::to_value(j)?
            }
            SignerCommand::List => serde_json::to_value(client.list_signers()?)?,
            SignerCommand::Unload { signer } => {
                let r = client.unload_signer(signer)?;
                serde_json::to_value(r)?
            }
            SignerCommand::SinglesigDesc {
                signer,
                descriptor_blinding_key,
                kind,
            } => {
                let r = client.singlesig_descriptor(
                    signer,
                    descriptor_blinding_key.to_string(),
                    kind.to_string(),
                )?;
                serde_json::to_value(r)?
            }
            SignerCommand::Xpub { signer, kind } => {
                let r = client.xpub(signer, kind.to_string())?;
                serde_json::to_value(r)?
            }
            SignerCommand::RegisterMultisig { signer, wallet } => {
                let r = client.register_multisig(signer, wallet)?;
                serde_json::to_value(r)?
            }
        },
        CliCommand::Wallet(a) => match a.command {
            WalletCommand::Load { descriptor, wallet } => {
                let r = client.load_wallet(descriptor, wallet)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Unload { wallet } => {
                let r = client.unload_wallet(wallet)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Balance {
                wallet,
                with_tickers,
            } => {
                let r = client.balance(wallet, with_tickers)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Send {
                wallet,
                recipient,
                fee_rate,
            } => {
                let mut addressees = vec![];
                for rec in recipient {
                    addressees.push(
                        rec.try_into()
                            .with_context(|| "error parsing recipient argument")?,
                    );
                }

                let r = client.send_many(wallet, addressees, fee_rate)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Address { index, wallet } => {
                let r = client.address(wallet, index)?;
                serde_json::to_value(r)?
            }
            WalletCommand::List => serde_json::to_value(client.list_wallets()?)?,
            WalletCommand::Issue {
                wallet,
                satoshi_asset,
                address_asset,
                satoshi_token,
                address_token,
                contract,
                fee_rate,
            } => {
                let r = client.issue(
                    wallet,
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
                wallet,
                asset,
                satoshi_asset,
                address_asset,
                fee_rate,
            } => {
                let r = client.reissue(wallet, asset, satoshi_asset, address_asset, fee_rate)?;
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
                wallet,
            } => {
                let r = client.broadcast(wallet, dry_run, pset)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Details { wallet } => {
                let r = client.wallet_details(wallet)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Combine { wallet, pset } => {
                let r = client.wallet_combine(wallet, pset)?;
                serde_json::to_value(r)?
            }
            WalletCommand::PsetDetails {
                wallet,
                pset,
                with_tickers,
            } => {
                let r = client.wallet_pset_details(wallet, pset, with_tickers)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Utxos { wallet } => {
                let r = client.wallet_utxos(wallet)?;
                serde_json::to_value(r)?
            }
            WalletCommand::Txs {
                wallet,
                with_tickers,
            } => {
                let r = client.wallet_txs(wallet, with_tickers)?;
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

        #[cfg(feature = "bindings")]
        CliCommand::Generate { .. } => {
            uniffi::uniffi_bindgen_main();
            Value::Null
        }
    })
}
