use std::{fs::File, sync::mpsc::RecvTimeoutError, time::Duration};

use anyhow::{anyhow, Context};
use app::config::Config;
use serde_json::Value;
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

use crate::args::{CliCommand, Network, ServerCommand, SignerCommand, WalletCommand};
pub use args::Cli;

mod args;

pub fn inner_main(args: args::Cli) -> anyhow::Result<Value> {
    let file = File::options()
        .create(true)
        .append(true)
        .open("debug.log")
        .expect("must have write permission");
    let (appender, _guard) = if args.stderr {
        tracing_appender::non_blocking(std::io::stderr())
    } else {
        tracing_appender::non_blocking(file)
    };
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_writer(appender)
        .finish();
    match tracing::subscriber::set_global_default(subscriber) {
        Ok(_) => tracing::info!(
            "logging initialized on {}",
            if args.stderr { "stderr" } else { "debug.log" }
        ),
        Err(_) => tracing::debug!("logging already initialized"),
    }

    tracing::info!("CLI initialized with args: {:?}", args);

    // start the app with default host/port
    let config = match args.network {
        Network::Mainnet => Config::default_mainnet(),
        Network::Testnet => Config::default_testnet(),
        Network::Regtest => Config::default_regtest(
            &args
                .electrum_url
                .ok_or_else(|| anyhow!("on regtest you have to specify --electrum-url"))?,
        ),
    };
    let mut app = app::App::new(config)?;
    // get a client to make requests
    let client = app.client()?;

    // verify the server is up
    if let CliCommand::Server(args::ServerArgs {
        command: ServerCommand::Start,
    }) = args.command
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
                    ctrlc::set_handler(move || {
                        tx.send(()).expect("Could not send signal on channel.")
                    })
                    .expect("Error setting Ctrl-C handler");

                    app.run().with_context(|| {
                        format!(
                            "Cannot start the server at \"{}\". It is probably already running.",
                            app.addr()
                        )
                    })?;
                    // get the app version
                    let version = client.version()?.version;
                    tracing::info!("App running version {}", version);

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

                    app.join_threads()?;
                    tracing::debug!("Threads ended");
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
            SignerCommand::Sign => todo!(),
            SignerCommand::Load { mnemonic, name } => {
                let j: app::model::SignerResponse = client.load_signer(mnemonic, name)?;
                serde_json::to_value(j)?
            }
            SignerCommand::List => serde_json::to_value(client.list_signers()?)?,
            SignerCommand::Unload { name } => {
                let r = client.unload_signer(name)?;
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
            WalletCommand::Balance { name } => {
                let r = client.balance(name)?;
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
            WalletCommand::Issue {} => todo!(),
            WalletCommand::Reissue {} => todo!(),
        },
    })
}

#[cfg(test)]
pub mod test {
    use clap::Parser;
    use elements::pset::PartiallySignedTransaction;
    use serde_json::Value;

    use crate::{args::Cli, inner_main};

    #[track_caller]
    fn sh_result(command: &str) -> anyhow::Result<Value> {
        let shell_words = shellwords::split(command).unwrap();
        let mut cli = Cli::try_parse_from(shell_words).unwrap();
        cli.stderr = std::env::var("RUST_LOG").is_ok();
        // cli.network = Network::Regtest;
        inner_main(cli)
    }

    #[track_caller]
    pub fn sh(command: &str) -> Value {
        sh_result(command).unwrap()
    }

    #[test]
    fn test_commands() {
        // This test use json `Value` so that changes in the model are noticed

        std::thread::spawn(|| {
            sh("cli server start");
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        let result = sh("cli signer generate");
        assert!(result.get("mnemonic").is_some());

        let result = sh("cli wallet list");
        let wallets = result.get("wallets").unwrap();
        assert!(wallets.as_array().unwrap().is_empty());

        let desc = "ct(L3jXxwef3fpB7hcrFozcWgHeJCPSAFiZ1Ji2YJMPxceaGvy3PC1q,elwpkh(tpubD6NzVbkrYhZ4Was8nwnZi7eiWUNJq2LFpPSCMQLioUfUtT1e72GkRbmVeRAZc26j5MRUz2hRLsaVHJfs6L7ppNfLUrm9btQTuaEsLrT7D87/*))#lrwadl63";
        let result = sh(&format!("cli wallet load --name custody {desc}"));
        assert_eq!(result.get("descriptor").unwrap().as_str().unwrap(), desc);

        let result = sh_result("cli wallet load --name custody anything");
        assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'custody' is already loaded"));

        let result = sh_result(&format!("cli wallet load --name differentname {desc}"));
        assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'custody' is already loaded"));

        let result = sh_result("cli wallet load --name wrong wrong");
        assert!(format!("{:?}", result.unwrap_err())
            .contains("Invalid descriptor: Not a CT Descriptor"));

        let result = sh("cli wallet list");
        let wallets = result.get("wallets").unwrap();
        assert!(!wallets.as_array().unwrap().is_empty());

        let result = sh("cli wallet balance --name custody");
        let balance_obj = result.get("balance").unwrap();
        let asset = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";
        let policy_obj = balance_obj.get(asset).unwrap();
        assert_eq!(policy_obj.as_number().unwrap().as_u64().unwrap(), 100000);

        let result = sh_result("cli wallet balance --name notexist");
        assert!(format!("{:?}", result.unwrap_err()).contains("Wallet 'notexist' does not exist"));

        let result = sh("cli wallet address --name custody");
        assert_eq!(result.get("address").unwrap().as_str().unwrap(), "tlq1qqdtwgfchn6rtl8peyw6afhrkpphqlyxls04vlwycez2fz6l7chlhxr8wtvy9s2v34f9sk0e2g058p0dwdp9kj2z8k7l7ewsnu");
        assert_eq!(result.get("index").unwrap().as_u64().unwrap(), 1);

        let result = sh("cli wallet address --name custody --index 0");
        assert_eq!(result.get("address").unwrap().as_str().unwrap(), "tlq1qqg0nthgrrl4jxeapsa40us5d2wv4ps2y63pxwqpf3zk6y69jderdtzfyr95skyuu3t03sh0fvj09f9xut8erjypuqfev6wuwh");
        assert_eq!(result.get("index").unwrap().as_u64().unwrap(), 0);

        let result = sh("cli wallet send --name custody --recipient tlq1qqwf6dzkyrukfzwmx3cxdpdx2z3zspgda0v7x874cewkucajdzrysa7z9fy0qnjvuz0ymqythd6jxy9d2e8ajka48efakgrp9t:2:144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49");
        let pset = result.get("pset").unwrap().as_str().unwrap();
        let _: PartiallySignedTransaction = pset.parse().unwrap();

        let result = sh("cli wallet unload --name custody");
        let unloaded = result.get("unloaded").unwrap();
        assert_eq!(unloaded.get("descriptor").unwrap().as_str().unwrap(), desc);
        assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "custody");

        let result = sh("cli wallet list");
        let wallets = result.get("wallets").unwrap();
        assert!(wallets.as_array().unwrap().is_empty());

        let result = sh("cli signer list");
        let signers = result.get("signers").unwrap();
        assert!(signers.as_array().unwrap().is_empty());

        let result = sh("cli signer generate");
        let mnemonic = result.get("mnemonic").unwrap().as_str().unwrap();
        let result = sh(&format!(
            r#"cli signer load --mnemonic "{mnemonic}" --name ss "#
        ));
        assert_eq!(result.get("name").unwrap().as_str().unwrap(), "ss");

        let result = sh_result(&format!(
            r#"cli signer load --mnemonic "{mnemonic}" --name ss2 "#
        ));
        assert!(format!("{:?}", result.unwrap_err()).contains("Signer 'ss' is already loaded"));

        let result = sh("cli signer list");
        let signers = result.get("signers").unwrap();
        assert!(!signers.as_array().unwrap().is_empty());

        let result = sh("cli signer unload --name ss");
        let unloaded = result.get("unloaded").unwrap();
        assert_eq!(unloaded.get("name").unwrap().as_str().unwrap(), "ss");

        let result = sh("cli signer list");
        let signers = result.get("signers").unwrap();
        assert!(signers.as_array().unwrap().is_empty());

        sh("cli server stop");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
