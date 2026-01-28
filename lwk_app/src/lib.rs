#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! # App
//!
//! Contains the RPC server [`App`] wiring the RPC calls to the respective methods in [`Wollet`] or [`Signer`].
//! The server can be configured via the [`Config`] struct and a convenient [cli](../cli/index.html) exists to call it.
//!
//! It also contains the RPC client [`Client`].
//!
//! Both the client and the server share the possible [`Error`]s.
//!
//! All the requests and responses data model are in the [`lwk_rpc_model`] crate.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::num::NonZeroU8;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, JoinHandle};
use std::time::Duration;

use lwk_common::{
    address_to_qr, address_to_text_qr, keyorigin_xpub_from_str, multisig_desc, singlesig_desc,
    InvalidBipVariant, InvalidBlindingKeyVariant, InvalidMultisigVariant, InvalidSinglesigVariant,
    Signer,
};
use lwk_jade::derivation_path_to_vec;
use lwk_jade::get_receive_address::Variant;
use lwk_jade::register_multisig::{JadeDescriptor, RegisterMultisigParams};
use lwk_jade::Jade;
use lwk_signer::{AnySigner, SwSigner};
use lwk_tiny_jrpc::{tiny_http, JsonRpcServer, Request, Response};
use lwk_wollet::amp2::Amp2;
use lwk_wollet::bitcoin::bip32::Fingerprint;
use lwk_wollet::bitcoin::XKeyIdentifier;
use lwk_wollet::clients::blocking::BlockchainBackend;
use lwk_wollet::elements::encode::serialize;
use lwk_wollet::elements::hex::{FromHex, ToHex};
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{Address, AssetId, OutPoint, Txid};
use lwk_wollet::elements_miniscript::descriptor::{Descriptor, DescriptorType, WshInner};
use lwk_wollet::elements_miniscript::miniscript::decode::Terminal;
use lwk_wollet::elements_miniscript::{DescriptorPublicKey, ForEachKey};
use lwk_wollet::registry::add_contracts;
use lwk_wollet::LiquidexProposal;
use lwk_wollet::Wollet;
use lwk_wollet::WolletDescriptor;
use serde_json::Value;

use crate::method::Method;
use crate::state::{AppAsset, AppSigner, State};
use lwk_rpc_model::{request, response};

pub use client::Client;
pub use config::Config;
pub use error::Error;
pub use lwk_tiny_jrpc::RpcError;

mod blockchain_client;
mod client;
mod config;
pub mod consts;
mod error;
pub mod method;
mod reqwest_transport;
mod state;

pub struct App {
    rpc: Option<JsonRpcServer>,
    config: Config,

    /// Set to false to stop the background scanning thread
    is_scanning: Arc<AtomicBool>,

    /// Handle of the scanning thread
    scanning_handle: Option<JoinHandle<()>>,
}

impl App {
    pub fn new(config: Config) -> Result<App, Error> {
        log::info!("Creating new app with config: {config:?}");

        Ok(App {
            rpc: None,
            config,
            scanning_handle: None,
            is_scanning: Arc::new(AtomicBool::new(false)),
        })
    }

    fn apply_request(&self, client: &Client, line: &str) -> Result<(), Error> {
        let r: Request = serde_json::from_str(line)?;
        let method: Method = r.method.parse()?;
        let _value: Value = client.make_request(method, r.params)?;
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Error> {
        if self.rpc.is_some() {
            return Err(error::Error::AlreadyStarted);
        }
        let mut state = State {
            config: self.config.clone(),
            wollets: Default::default(),
            signers: Default::default(),
            assets: Default::default(),
            tx_memos: Default::default(),
            addr_memos: Default::default(),
            do_persist: false,
            scan_loops_started: 0,
            scan_loops_completed: 0,
            interrupt_wait: false,
        };
        state.insert_policy_asset();
        let state = Arc::new(Mutex::new(state));
        let server = tiny_http::Server::http(self.config.addr)
            .map_err(|_| Error::ServerStart(self.config.addr.to_string()))?;

        // TODO, for some reasons, using the default number of threads (4) cause a request to be
        // replied after 15 seconds, using 1 instead seems to not have that issue.
        let config = lwk_tiny_jrpc::Config::builder()
            .with_num_threads(NonZeroU8::new(1).expect("static"))
            .build();

        let rpc = lwk_tiny_jrpc::JsonRpcServer::new(server, config, state.clone(), method_handler);
        let path = self.config.state_path()?;
        match std::fs::read_to_string(&path) {
            Ok(string) => {
                log::info!(
                    "Loading previous state, {} elements",
                    string.lines().count()
                );

                let client = self.client()?;

                for (n, line) in string.lines().enumerate() {
                    self.apply_request(&client, line).map_err(|err| {
                        Error::StartStateLoad(err.to_string(), n + 1, path.display().to_string())
                    })?
                }
            }
            Err(_) => {
                log::info!("There is no previous state at {path:?}");
            }
        }
        state.lock().map_err(|e| e.to_string())?.do_persist = true;

        self.rpc = Some(rpc);

        // Wallets scanning thread
        self.is_scanning.store(true, Ordering::Relaxed);
        let is_scanning = self.is_scanning.clone();
        let state_scanning = state.clone();
        let scanning_interval = self.config.scanning_interval;
        let stop_interval = Duration::from_millis(100);
        let mut interval = Duration::ZERO; // Do not wait in the first scan loop
        let scanning_handle = std::thread::spawn(move || 'scan: loop {
            // Sleep for scanning_interval, but check stop signal every stop_interval
            'stop: loop {
                if !is_scanning.load(Ordering::Relaxed) {
                    break 'scan;
                }
                if interval == Duration::ZERO
                    || state_scanning
                        .lock()
                        .map(|s| s.interrupt_wait)
                        .unwrap_or(false)
                {
                    interval = scanning_interval; // Reset wait interval
                    break 'stop;
                }
                std::thread::sleep(stop_interval);
                interval = interval.saturating_sub(stop_interval);
            }

            let (wollets_names, config) = {
                let mut s = state_scanning.lock().expect("state lock poison");
                s.interrupt_wait = false;
                s.scan_loops_started += 1;
                let wollets_names: Vec<_> = s.wollets.iter().map(|e| e.0.to_owned()).collect();
                let config = s.config.clone();
                (wollets_names, config)
            };

            match config.blockchain_client() {
                Ok(mut blockchain_client) => {
                    for name in wollets_names {
                        let state = match state_scanning
                            .lock()
                            .expect("state lock poison")
                            .wollets
                            .get(&name)
                        {
                            Ok(w) => w.state(),
                            Err(_) => continue,
                        };

                        match blockchain_client.full_scan(&state) {
                            Ok(Some(update)) => {
                                let mut s = state_scanning.lock().expect("state lock poison");
                                let _ = match s.wollets.get_mut(&name) {
                                    Ok(wollet) => wollet.apply_update(update),
                                    Err(_) => continue,
                                };
                            }
                            Ok(None) => (),
                            Err(_) => continue,
                        }
                    }
                }
                Err(_) => {
                    log::info!(
                        "Cannot create an electrum client, are we conected? Retrying in one sec"
                    );
                    sleep(Duration::from_secs(1))
                }
            };

            let mut s = state_scanning.lock().expect("state lock poison");
            s.scan_loops_completed += 1;
        });
        self.scanning_handle = Some(scanning_handle);

        Ok(())
    }

    pub fn stop(&self) -> Result<(), Error> {
        self.is_scanning.store(false, Ordering::Relaxed);
        match self.rpc.as_ref() {
            Some(rpc) => {
                rpc.stop();
                Ok(())
            }
            None => Err(error::Error::NotStarted),
        }
    }

    pub fn is_running(&self) -> Result<bool, Error> {
        match self.rpc.as_ref() {
            Some(rpc) => Ok(rpc.is_running()),
            None => Err(error::Error::NotStarted),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.config.addr
    }

    pub fn join_threads(&mut self) -> Result<(), Error> {
        self.rpc
            .take()
            .ok_or(error::Error::NotStarted)?
            .join_threads();
        if let Some(scanning_handle) = self.scanning_handle.take() {
            let _ = scanning_handle.join();
        }
        Ok(())
    }

    fn client(&self) -> Result<Client, Error> {
        Client::new(self.config.addr)
    }
}

fn method_handler(
    request: Request,
    state: Arc<Mutex<State>>,
) -> Result<Response, lwk_tiny_jrpc::Error> {
    Ok(inner_method_handler(request, state)?)
}

fn inner_method_handler(request: Request, state: Arc<Mutex<State>>) -> Result<Response, Error> {
    log::debug!(
        "method: {} params: {:?} ",
        request.method.as_str(),
        request.params
    );
    let method: Method = match request.method.as_str().parse() {
        Ok(method) => method,
        Err(e) => return Ok(Response::unimplemented(request.id, e.to_string())),
    };

    // TODO to remove the clone:
    // 1) refactor out AppState wallets/signers/assets conversion to Requests in as_requests
    // 2) use that in the persist() calls
    let params = request.params.clone().unwrap_or_default();

    let response = match method {
        Method::Schema => {
            let r: request::Schema = serde_json::from_value(params)?;
            let method: Method = r.method.parse()?;
            Response::result(request.id, method.schema(r.direction)?)
        }
        Method::SignerGenerate => {
            let (_signer, mnemonic) = SwSigner::random(state.lock()?.config.is_mainnet())?;
            Response::result(
                request.id,
                serde_json::to_value(response::SignerGenerate {
                    mnemonic: mnemonic.to_string(),
                })?,
            )
        }
        Method::Version => {
            let network = state.lock()?.config.network.as_str().to_string();
            Response::result(
                request.id,
                serde_json::to_value(response::Version {
                    version: consts::APP_VERSION.into(),
                    network,
                })?,
            )
        }
        Method::WalletLoad => {
            let r: request::WalletLoad = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            // TODO recognize different name same descriptor?

            let desc: WolletDescriptor = r.descriptor.parse()?;
            if desc.is_mainnet() != s.config.is_mainnet() {
                return Err(Error::Generic("Descriptor is for the wrong network".into()));
            }
            let wollet = Wollet::with_fs_persist(s.config.network, desc, &s.config.datadir)?;
            s.wollets.insert(&r.name, wollet)?;

            s.persist(&request)?;

            Response::result(
                request.id,
                serde_json::to_value(response::Wallet {
                    descriptor: r.descriptor,
                    name: r.name,
                })?,
            )
        }
        Method::WalletUnload => {
            let r: request::WalletUnload = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let removed = s.wollets.remove(&r.name)?;
            s.tx_memos.remove(&r.name);
            s.addr_memos.remove(&r.name);
            s.persist_all()?;

            Response::result(
                request.id,
                serde_json::to_value(response::WalletUnload {
                    unloaded: response::Wallet {
                        name: r.name,
                        descriptor: removed.descriptor()?.to_string(),
                    },
                })?,
            )
        }
        Method::WalletList => {
            let s = state.lock()?;
            let wallets = s
                .wollets
                .iter()
                .map(|(name, wollet)| response::Wallet {
                    descriptor: wollet.wollet_descriptor().to_string(),
                    name: name.clone(),
                })
                .collect();
            let r = response::WalletList { wallets };
            Response::result(request.id, serde_json::to_value(r)?)
        }
        Method::SignerLoadSoftware => {
            let r: request::SignerLoadSoftware = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let signer = AppSigner::new_sw(&r.mnemonic, s.config.is_mainnet(), r.persist)?;
            let resp: response::Signer = signer_response_from(&r.name, &signer)?;
            s.signers.insert(&r.name, signer)?;
            if r.persist {
                s.persist(&request)?;
            }
            Response::result(request.id, serde_json::to_value(resp)?)
        }
        Method::SignerLoadJade => {
            let r: request::SignerLoadJade = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let id = XKeyIdentifier::from_str(&r.id).map_err(|e| e.to_string())?; // TODO remove map_err
            let signer = AppSigner::new_jade(id, r.emulator, s.config.jade_network())?;
            let resp: response::Signer = signer_response_from(&r.name, &signer)?;
            s.signers.insert(&r.name, signer)?;
            s.persist(&request)?;
            Response::result(request.id, serde_json::to_value(resp)?)
        }
        Method::SignerLoadExternal => {
            let r: request::SignerLoadExternal = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let fingerprint =
                Fingerprint::from_str(&r.fingerprint).map_err(|e| Error::Generic(e.to_string()))?;
            let signer = AppSigner::new_external(fingerprint);
            let resp: response::Signer = signer_response_from(&r.name, &signer)?;
            s.signers.insert(&r.name, signer)?;
            s.persist(&request)?;
            Response::result(request.id, serde_json::to_value(resp)?)
        }
        Method::SignerUnload => {
            let r: request::SignerUnload = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let removed = s.signers.remove(&r.name)?;
            let signer: response::Signer = signer_response_from(&r.name, &removed)?;
            s.persist_all()?;
            Response::result(
                request.id,
                serde_json::to_value(response::SignerUnload { unloaded: signer })?,
            )
        }
        Method::SignerDetails => {
            let r: request::SignerDetails = serde_json::from_value(params)?;
            let s = state.lock()?;
            let signer = s.signers.get(&r.name)?;
            let details = signer_details(&r.name, signer)?;
            Response::result(request.id, serde_json::to_value(details)?)
        }
        Method::SignerList => {
            let s = state.lock()?;
            let signers: Result<Vec<_>, _> = s
                .signers
                .iter()
                .map(|(name, signer)| signer_response_from(name, signer))
                .collect();
            let mut signers = signers?;
            signers.sort();
            let r = response::SignerList { signers };
            Response::result(request.id, serde_json::to_value(r)?)
        }
        Method::WalletAddress => {
            let r: request::WalletAddress = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let wollet = s.wollets.get_mut(&r.name)?;
            let addr = wollet.address(r.index)?;
            let definite_desc = wollet
                .wollet_descriptor()
                .definite_descriptor(lwk_wollet::Chain::External, addr.index())?;

            let text_qr = r
                .with_text_qr
                .then(|| address_to_text_qr(addr.address()))
                .transpose()?;
            let uri_qr = r
                .with_uri_qr
                .map(|e| {
                    let pixel_per_module = (e != 0).then_some(e);
                    address_to_qr(addr.address(), pixel_per_module)
                })
                .transpose()?;

            if let Some(signer) = r.signer {
                let signer = s.get_available_signer(&signer)?;
                if let AnySigner::Jade(jade, _id) = signer {
                    let fingerprint = signer.fingerprint()?;

                    // Get the derivation paths for all signers
                    let mut paths: Vec<Vec<u32>> = vec![];
                    // Get the full path for the signer
                    let mut full_path: Vec<u32> = vec![];
                    definite_desc.for_each_key(|k| {
                        if k.master_fingerprint() == fingerprint {
                            if let Some(path) = k.full_derivation_path() {
                                full_path = derivation_path_to_vec(&path);
                            }
                        }
                        if let DescriptorPublicKey::XPub(x) = k.as_descriptor_public_key() {
                            paths.push(derivation_path_to_vec(&x.derivation_path));
                        }
                        true
                    });

                    if full_path.is_empty() {
                        return Err(Error::Generic("Signer is not in wallet".into()));
                    }
                    let jade_addr = match paths.len() {
                        0 => return Err(Error::Generic("Unsupported signer or descriptor".into())),
                        1 => {
                            // Single sig
                            match definite_desc.desc_type() {
                                DescriptorType::Wpkh => {
                                    jade.get_receive_address_single(Variant::Wpkh, full_path)?
                                }
                                DescriptorType::ShWpkh => {
                                    jade.get_receive_address_single(Variant::ShWpkh, full_path)?
                                }
                                _ => {
                                    return Err(Error::Generic(
                                        "Unsupported signer or descriptor".into(),
                                    ))
                                }
                            }
                        }
                        _ => {
                            // Multi sig
                            jade.get_receive_address_multi(&r.name, paths)?
                        }
                    };
                    if jade_addr != addr.address().to_string() {
                        return Err(Error::Generic(
                            "Mismatching addresses between wallet and jade".into(),
                        ));
                    }
                } else {
                    return Err(Error::Generic(
                        "Cannot display address with software signer".into(),
                    ));
                }
            };

            let address = addr.address();
            let memos = s.addr_memos.for_wollet(&r.name);
            let memo = memos.get(address).cloned().unwrap_or_default();
            Response::result(
                request.id,
                serde_json::to_value(response::WalletAddress {
                    address: address.to_string(),
                    index: addr.index(),
                    memo,
                    text_qr,
                    uri_qr,
                })?,
            )
        }
        Method::WalletBalance => {
            let r: request::WalletBalance = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            let mut balance = wollet
                .balance()?
                .as_ref()
                .clone()
                .into_iter()
                .map(|(k, v)| (k.to_string(), v as i64))
                .collect();
            if r.with_tickers {
                balance = s.replace_id_with_ticker(balance);
            }
            Response::result(
                request.id,
                serde_json::to_value(response::WalletBalance { balance })?,
            )
        }
        Method::WalletSendMany => {
            let r: request::WalletSendMany = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet: &mut Wollet = s.wollets.get_mut(&r.name)?;

            let recipients: Vec<_> = r
                .addressees
                .into_iter()
                .map(unvalidated_addressee)
                .collect();
            let builder = wollet
                .tx_builder()
                .set_unvalidated_recipients(&recipients)?
                .fee_rate(r.fee_rate);
            let mut tx = builder.finish()?;

            add_contracts(&mut tx, s.registry_asset_data());
            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: tx.to_string(),
                })?,
            )
        }
        Method::WalletDrain => {
            let r: request::WalletDrain = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet: &mut Wollet = s.wollets.get_mut(&r.name)?;

            let address = Address::from_str(&r.address)?;
            let mut tx = wollet
                .tx_builder()
                .drain_lbtc_wallet()
                .drain_lbtc_to(address)
                .fee_rate(r.fee_rate)
                .finish()?;

            add_contracts(&mut tx, s.registry_asset_data());
            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: tx.to_string(),
                })?,
            )
        }
        Method::SignerSinglesigDescriptor => {
            let r: request::SignerSinglesigDescriptor = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let signer = s.get_available_signer(&r.name)?;

            let script_variant = r
                .singlesig_kind
                .parse()
                .map_err(|e: InvalidSinglesigVariant| e.to_string())?;

            let blinding_variant = r
                .descriptor_blinding_key
                .parse()
                .map_err(|e: InvalidBlindingKeyVariant| e.to_string())?;

            let descriptor = singlesig_desc(signer, script_variant, blinding_variant)?;
            Response::result(
                request.id,
                serde_json::to_value(response::SignerSinglesigDescriptor { descriptor })?,
            )
        }
        Method::WalletMultisigDescriptor => {
            let r: request::WalletMultisigDescriptor = serde_json::from_value(params)?;

            let multisig_variant = r
                .multisig_kind
                .parse()
                .map_err(|e: InvalidMultisigVariant| e.to_string())?;

            let blinding_variant = r
                .descriptor_blinding_key
                .parse()
                .map_err(|e: InvalidBlindingKeyVariant| e.to_string())?;

            let mut keyorigin_xpubs = vec![];
            for keyorigin_xpub in r.keyorigin_xpubs {
                keyorigin_xpubs.push(
                    keyorigin_xpub_from_str(&keyorigin_xpub)
                        .map_err(|e| Error::Generic(e.to_string()))?,
                );
            }

            let descriptor = multisig_desc(
                r.threshold,
                keyorigin_xpubs,
                multisig_variant,
                blinding_variant,
            )?;
            Response::result(
                request.id,
                serde_json::to_value(response::WalletMultisigDescriptor { descriptor })?,
            )
        }
        Method::SignerRegisterMultisig => {
            let r: request::SignerRegisterMultisig = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let network = s.config.jade_network();
            let descriptor = s.wollets.get(&r.wallet)?.descriptor()?.clone();
            let signer = s.get_available_signer(&r.name)?;

            if let AnySigner::Jade(jade, _id) = signer {
                let descriptor: JadeDescriptor = (&descriptor).try_into()?;
                jade.register_multisig(RegisterMultisigParams {
                    network,
                    multisig_name: r.wallet,
                    descriptor,
                })?;
            }
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::SignerXpub => {
            let r: request::SignerXpub = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let is_mainnet = s.config.is_mainnet();

            let signer = s.get_available_signer(&r.name)?;

            let bip = r
                .xpub_kind
                .parse()
                .map_err(|e: InvalidBipVariant| e.to_string())?;

            let keyorigin_xpub = signer.keyorigin_xpub(bip, is_mainnet)?;
            Response::result(
                request.id,
                serde_json::to_value(response::SignerXpub { keyorigin_xpub })?,
            )
        }
        Method::SignerSign => {
            let r: request::SignerSign = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let signer = s.get_available_signer(&r.name)?;

            let mut pset =
                PartiallySignedTransaction::from_str(&r.pset).map_err(|e| e.to_string())?;

            signer.sign(&mut pset)?;

            // TODO we may want to return other details such as if signatures have been added

            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: pset.to_string(),
                })?,
            )
        }
        Method::SignerDeriveBip85 => {
            let r: request::SignerDeriveBip85 = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let signer = s.get_available_signer(&r.name)?;

            // Only software signers support BIP85 derivation
            let sw_signer = match signer {
                AnySigner::Software(sw) => sw,
                _ => {
                    return Err(Error::Generic(
                        "BIP85 derivation is only supported for software signers".to_string(),
                    ))
                }
            };

            let derived_mnemonic = sw_signer
                .derive_bip85_mnemonic(r.index, r.word_count)
                .map_err(|e| Error::Generic(format!("BIP85 derivation failed: {e}")))?;

            Response::result(
                request.id,
                serde_json::to_value(response::SignerDeriveBip85 {
                    mnemonic: derived_mnemonic.to_string(),
                })?,
            )
        }
        Method::WalletBroadcast => {
            let r: request::WalletBroadcast = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let wollet = s.wollets.get_mut(&r.name)?;
            let mut pset =
                PartiallySignedTransaction::from_str(&r.pset).map_err(|e| e.to_string())?;
            let tx = wollet.finalize(&mut pset)?;
            let blockchain_client = s.config.blockchain_client()?;

            if !r.dry_run {
                blockchain_client.broadcast(&tx)?;
            }

            Response::result(
                request.id,
                serde_json::to_value(response::WalletBroadcast {
                    txid: tx.txid().to_string(),
                })?,
            )
        }
        Method::WalletDetails => {
            let r: request::WalletDetails = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;

            let descriptor = wollet.descriptor()?.to_string();
            let type_ = match wollet.descriptor()?.descriptor.desc_type() {
                DescriptorType::Wpkh => response::WalletType::Wpkh,
                DescriptorType::ShWpkh => response::WalletType::ShWpkh,
                _ => match &wollet.descriptor()?.descriptor {
                    Descriptor::Wsh(wsh) => match wsh.as_inner() {
                        WshInner::Ms(ms) => match &ms.node {
                            Terminal::Multi(threshold, pubkeys) => {
                                response::WalletType::WshMulti(*threshold, pubkeys.len())
                            }
                            _ => response::WalletType::Unknown,
                        },
                        _ => response::WalletType::Unknown,
                    },
                    _ => response::WalletType::Unknown,
                },
            };

            let mut warnings: Vec<String> = vec![];

            let has_unique_fingerprints = {
                let mut hs = HashSet::new();
                wollet.signers().into_iter().all(|f| hs.insert(f))
            };
            if !has_unique_fingerprints {
                warnings.push("wallet has multiple signers with the same fingerprint".into());
            }

            let signers: Vec<_> = wollet
                .signers()
                .iter()
                .map(|fingerprint| {
                    let name = s.signers.name_from_fingerprint(fingerprint, &mut warnings);
                    response::SignerShortDetails {
                        name,
                        fingerprint: fingerprint.to_string(),
                    }
                })
                .collect();

            Response::result(
                request.id,
                serde_json::to_value(response::WalletDetails {
                    descriptor,
                    type_: type_.to_string(),
                    signers,
                    warnings: warnings.join(", "),
                })?,
            )
        }
        Method::WalletCombine => {
            let r: request::WalletCombine = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;

            let mut psets = vec![];
            for pset in r.pset {
                psets.push(PartiallySignedTransaction::from_str(&pset).map_err(|e| e.to_string())?);
            }
            let pset = wollet.combine(&psets)?;
            Response::result(
                request.id,
                serde_json::to_value(response::WalletCombine {
                    pset: pset.to_string(),
                })?,
            )
        }
        Method::WalletPsetDetails => {
            let r: request::WalletPsetDetails = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;

            let pset = PartiallySignedTransaction::from_str(&r.pset).map_err(|e| e.to_string())?;
            let details = wollet.get_details(&pset)?;
            let mut warnings = vec![];
            let has_signatures_from = details
                .fingerprints_has()
                .iter()
                .map(|f| response::SignerShortDetails {
                    name: s.signers.name_from_fingerprint(f, &mut warnings),
                    fingerprint: f.to_string(),
                })
                .collect();
            let missing_signatures_from = details
                .fingerprints_missing()
                .iter()
                .map(|f| response::SignerShortDetails {
                    name: s.signers.name_from_fingerprint(f, &mut warnings),
                    fingerprint: f.to_string(),
                })
                .collect();
            let mut balance: HashMap<String, i64> = details
                .balance
                .balances
                .as_ref()
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect();
            if r.with_tickers {
                balance = s.replace_id_with_ticker(balance);
            }
            let issuances = details
                .issuances
                .iter()
                .enumerate()
                .filter(|(_, e)| e.is_issuance())
                .map(|(vin, e)| response::Issuance {
                    asset: e.asset().expect("issuance").to_string(),
                    token: e.token().expect("issuance").to_string(),
                    is_confidential: e.is_confidential(),
                    vin: vin as u32,
                    asset_satoshi: e.asset_satoshi().unwrap_or(0),
                    token_satoshi: e.token_satoshi().unwrap_or(0),
                    prev_txid: e.prev_txid().expect("issuance").to_string(),
                    prev_vout: e.prev_vout().expect("issuance"),
                })
                .collect();
            let reissuances = details
                .issuances
                .iter()
                .enumerate()
                .filter(|(_, e)| e.is_reissuance())
                .map(|(vin, e)| response::Reissuance {
                    asset: e.asset().expect("reissuance").to_string(),
                    token: e.token().expect("reissuance").to_string(),
                    is_confidential: e.is_confidential(),
                    vin: vin as u32,
                    asset_satoshi: e.asset_satoshi().unwrap_or(0),
                })
                .collect();

            Response::result(
                request.id,
                serde_json::to_value(response::WalletPsetDetails {
                    has_signatures_from,
                    missing_signatures_from,
                    balance,
                    fee: details.balance.fee,
                    issuances,
                    reissuances,
                    warnings: warnings.join(", "),
                })?,
            )
        }
        Method::WalletUtxos => {
            let r: request::WalletUtxos = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            let utxos: Vec<response::Utxo> = wollet.utxos()?.iter().map(convert_utxo).collect();
            Response::result(
                request.id,
                serde_json::to_value(response::WalletUtxos { utxos })?,
            )
        }
        Method::WalletTxs => {
            let r: request::WalletTxs = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let explorer_url = s.config.explorer_url.clone();
            let memos = s.tx_memos.for_wollet(&r.name);
            let wollet = s.wollets.get_mut(&r.name)?;
            let mut txs: Vec<response::Tx> = wollet
                .transactions()?
                .iter()
                .map(|tx| convert_tx(tx, &explorer_url, &memos))
                .collect();
            if r.with_tickers {
                for tx in &mut txs {
                    tx.balance = s.replace_id_with_ticker(tx.balance.clone());
                }
            }
            Response::result(
                request.id,
                serde_json::to_value(response::WalletTxs { txs })?,
            )
        }
        Method::WalletTx => {
            let r: request::WalletTx = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            let txid = Txid::from_str(&r.txid)?;
            let tx = if let Some(tx) = wollet.transaction(&txid)? {
                tx.tx.clone()
            } else if r.fetch {
                let client = s.config.blockchain_client()?;
                let mut txs = client.get_transactions(&[txid])?;
                txs.pop().ok_or(Error::WalletTxNotFound(r.txid, r.name))?
            } else {
                return Err(Error::WalletTxNotFound(r.txid, r.name));
            };
            let tx = serialize(&tx).to_hex();
            Response::result(request.id, serde_json::to_value(response::WalletTx { tx })?)
        }
        Method::WalletSetTxMemo => {
            let r: request::WalletSetTxMemo = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            // Make sure the wallet exists
            let _wollet = s.wollets.get(&r.name)?;
            let txid = Txid::from_str(&r.txid).map_err(|e| Error::Generic(e.to_string()))?;
            s.tx_memos.set(&r.name, &txid, &r.memo)?;
            s.persist(&request)?;
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::WalletSetAddrMemo => {
            let r: request::WalletSetAddrMemo = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            // Make sure the wallet exists
            let _wollet = s.wollets.get(&r.name)?;
            let address =
                Address::from_str(&r.address).map_err(|e| Error::Generic(e.to_string()))?;
            // TODO: check address belongs to the wallet
            s.addr_memos.set(&r.name, &address, &r.memo)?;
            s.persist(&request)?;
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::LiquidexMake => {
            let r: request::LiquidexMake = serde_json::from_value(params)?;
            let txid = Txid::from_str(&r.txid)?;
            let vout = r.vout;
            let asset = AssetId::from_str(&r.asset)?;
            let satoshi = r.satoshi;

            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;

            let outpoint = OutPoint::new(txid, vout);

            let addr = wollet.address(Some(0))?;
            let receiving_address = addr.address().to_string();

            let receiving_address = Address::from_str(&receiving_address)?;

            let pset = wollet
                .tx_builder()
                .liquidex_make(outpoint, &receiving_address, satoshi, asset)?
                .finish()?;

            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: pset.to_string(),
                })?,
            )
        }
        Method::LiquidexTake => {
            let r: request::LiquidexTake = serde_json::from_value(params)?;

            let proposal = LiquidexProposal::from_str(&r.proposal)?;
            let txid = proposal.needed_tx()?;
            let client = {
                let s = state.lock()?;
                s.config.blockchain_client()?
            };
            let tx = client.get_transactions(&[txid])?.pop().expect("tx");
            let proposal = proposal.validate(tx)?;

            let mut s = state.lock()?;
            let wollet: &mut Wollet = s.wollets.get_mut(&r.name)?;

            let pset = wollet
                .tx_builder()
                .liquidex_take(vec![proposal])?
                .finish()?;

            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: pset.to_string(),
                })?,
            )
        }
        Method::LiquidexToProposal => {
            let r: request::LiquidexToProposal = serde_json::from_value(params)?;
            let pset = PartiallySignedTransaction::from_str(&r.pset).map_err(|e| e.to_string())?;
            let proposal = LiquidexProposal::from_pset(&pset)?;

            let proposal = serde_json::to_value(&proposal)?;
            Response::result(
                request.id,
                serde_json::to_value(response::LiquidexProposal { proposal })?,
            )
        }
        Method::WalletIssue => {
            let r: request::WalletIssue = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            let tx = wollet
                .tx_builder()
                .issue_asset(
                    r.satoshi_asset,
                    r.address_asset.map(|a| Address::from_str(&a)).transpose()?,
                    r.satoshi_token,
                    r.address_token.map(|a| Address::from_str(&a)).transpose()?,
                    r.contract
                        .map(|c| lwk_wollet::Contract::from_str(&c))
                        .transpose()?,
                )?
                .fee_rate(r.fee_rate)
                .finish()?;
            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: tx.to_string(),
                })?,
            )
        }
        Method::WalletReissue => {
            let r: request::WalletReissue = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let asset_id = AssetId::from_str(&r.asset)?;
            let issuance_tx = s.get_issuance_tx(&asset_id);
            let wollet = s.wollets.get_mut(&r.name)?;

            let mut pset = wollet
                .tx_builder()
                .reissue_asset(
                    asset_id,
                    r.satoshi_asset,
                    r.address_asset.map(|a| Address::from_str(&a)).transpose()?,
                    issuance_tx,
                )?
                .fee_rate(r.fee_rate)
                .finish()?;

            add_contracts(&mut pset, s.registry_asset_data());
            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: pset.to_string(),
                })?,
            )
        }
        Method::WalletBurn => {
            let r: request::WalletBurn = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let asset_id = AssetId::from_str(&r.asset)?;
            let wollet = s.wollets.get_mut(&r.name)?;

            let mut pset = wollet
                .tx_builder()
                .add_burn(r.satoshi_asset, asset_id)?
                .fee_rate(r.fee_rate)
                .finish()?;

            add_contracts(&mut pset, s.registry_asset_data());
            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: pset.to_string(),
                })?,
            )
        }
        Method::AssetContract => {
            let r: request::AssetContract = serde_json::from_value(params)?;
            let c = lwk_wollet::Contract {
                entity: lwk_wollet::Entity::Domain(r.domain),
                issuer_pubkey: Vec::<u8>::from_hex(&r.issuer_pubkey)?,
                name: r.name,
                precision: r.precision,
                ticker: r.ticker,
                version: r.version,
            };
            c.validate()?; // TODO: validation should be done at Contract creation

            Response::result(request.id, serde_json::to_value(c)?)
        }
        Method::AssetDetails => {
            let r: request::AssetDetails = serde_json::from_value(params)?;
            let s = state.lock()?;
            let asset_id = lwk_wollet::elements::AssetId::from_str(&r.asset_id)
                .map_err(|e| Error::Generic(e.to_string()))?;
            let asset = s.get_asset(&asset_id)?;
            Response::result(
                request.id,
                serde_json::to_value(response::AssetDetails {
                    name: asset.name(),
                    ticker: asset.ticker(),
                })?,
            )
        }
        Method::AssetList => {
            let s = state.lock()?;
            let mut assets: Vec<_> = s
                .assets
                .iter()
                .map(|(asset_id, asset)| response::Asset {
                    asset_id: asset_id.to_string(),
                    name: asset.name(),
                })
                .collect();
            assets.sort();
            let r = response::AssetList { assets };
            Response::result(request.id, serde_json::to_value(r)?)
        }
        Method::AssetInsert => {
            let r: request::AssetInsert = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let asset_id = lwk_wollet::elements::AssetId::from_str(&r.asset_id)
                .map_err(|e| Error::Generic(e.to_string()))?;
            let issuance_tx =
                Vec::<u8>::from_hex(&r.issuance_tx).map_err(|e| Error::Generic(e.to_string()))?;
            let issuance_tx = lwk_wollet::elements::encode::deserialize(&issuance_tx)
                .map_err(|e| Error::Generic(e.to_string()))?;
            let contract = serde_json::Value::from_str(&r.contract)?;
            let contract = lwk_wollet::Contract::from_value(&contract)?;
            s.insert_asset(asset_id, issuance_tx, contract)?;
            s.persist(&request)?;
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::AssetRemove => {
            let r: request::AssetRemove = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let asset_id = lwk_wollet::elements::AssetId::from_str(&r.asset_id)
                .map_err(|e| Error::Generic(e.to_string()))?;
            s.remove_asset(&asset_id)?;
            s.persist_all()?;
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::AssetFromRegistry => {
            let r: request::AssetFromRegistry = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let asset_id = AssetId::from_str(&r.asset_id)?;
            if s.get_asset(&asset_id).is_ok() {
                return Err(Error::AssetAlreadyInserted(r.asset_id));
            }
            let registry = lwk_wollet::registry::blocking::Registry::new(&s.config.registry_url)?;
            let (contract, issuance_tx) =
                registry.fetch_with_tx(asset_id, &s.config.blockchain_client()?)?;
            s.insert_asset(asset_id, issuance_tx, contract)?;
            // convert the request to an AssetInsert to skip network calls
            let asset_insert_request = s.get_asset(&asset_id)?.request().expect("asset");
            s.persist(&asset_insert_request)?;
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::SignerJadeId => {
            let r: request::SignerJadeId = serde_json::from_value(params)?;

            let (network, timeout) = {
                let s = state.lock()?;
                (s.config.jade_network(), Some(s.config.timeout))
            };
            log::debug!("jade network: {network}");

            let jade = match r.emulator {
                Some(emulator) => Jade::from_socket(emulator, network)?,
                #[cfg(not(feature = "serial"))]
                None => {
                    let _timeout = timeout;
                    return Err(Error::FeatSerialDisabled);
                }
                #[cfg(feature = "serial")]
                None => {
                    // TODO instead of the first working, we should return all the available jades with the port currently connected on
                    let mut jade = Jade::from_any_serial(network, timeout)
                        .into_iter()
                        .filter_map(|e| e.ok())
                        .next();
                    jade.take()
                        .ok_or(Error::Generic("no Jade available".to_string()))?
                }
            };
            jade.unlock()?;
            let identifier = jade.identifier()?.to_string();
            Response::result(
                request.id,
                serde_json::to_value(response::JadeId { identifier })?,
            )
        }
        Method::Scan => {
            scan(&state)?;
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::Stop => {
            return Err(Error::Stop);
        }
        Method::AssetPublish => {
            let r: request::AssetPublish = serde_json::from_value(params)?;
            let asset_id =
                AssetId::from_str(&r.asset_id).map_err(|e| Error::Generic(e.to_string()))?;
            let s = state.lock()?;
            let asset = s.get_asset(&asset_id)?;
            if let AppAsset::RegistryAsset(asset) = asset {
                let client = reqwest::blocking::Client::new();
                let url = &s.config.registry_url;
                let contract = asset.contract();
                let data = serde_json::json!({"asset_id": asset_id, "contract": contract});
                log::debug!("posting {data:?} as json to {url} ");
                let response = client.post(url).json(&data).send()?;
                let mut result = response.text()?;
                if result.contains("failed verifying linked entity") {
                    let domain = contract.entity.domain();
                    result = format!("https://{domain}/.well-known/liquid-asset-proof-{asset_id} must contain the following 'Authorize linking the domain name {domain} to the Liquid asset {asset_id}'");
                }
                Response::result(
                    request.id,
                    serde_json::to_value(response::AssetPublish {
                        asset_id: asset_id.to_string(),
                        result,
                    })?,
                )
            } else {
                return Err(Error::Generic(
                    "Can't publish a policy asset or a reissuance token".to_string(),
                ));
            }
        }
        Method::Amp2Descriptor => {
            let r: request::Amp2Descriptor = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            if !matches!(s.config.network, lwk_wollet::ElementsNetwork::LiquidTestnet) {
                return Err(Error::Generic(
                    "AMP2 methods are not available for this network".into(),
                ));
            }
            let signer = s.get_available_signer(&r.name)?;
            let amp2 = Amp2::new_testnet();
            let desc = amp2.descriptor_from_str(&amp2userkey(signer)?)?;
            let descriptor = desc.descriptor().to_string();
            Response::result(
                request.id,
                serde_json::to_value(response::Amp2Descriptor { descriptor })?,
            )
        }
        Method::Amp2Register => {
            let r: request::Amp2Register = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            if !matches!(s.config.network, lwk_wollet::ElementsNetwork::LiquidTestnet) {
                return Err(Error::Generic(
                    "AMP2 methods are not available for this network".into(),
                ));
            }
            let signer = s.get_available_signer(&r.name)?;
            let amp2 = Amp2::new_testnet();
            let desc = amp2.descriptor_from_str(&amp2userkey(signer)?)?;
            let wid = amp2.blocking_register(desc)?.wid;
            Response::result(
                request.id,
                serde_json::to_value(response::Amp2Register { wid })?,
            )
        }
        Method::Amp2Cosign => {
            let r: request::Amp2Cosign = serde_json::from_value(params)?;
            let s = state.lock()?;
            if !matches!(s.config.network, lwk_wollet::ElementsNetwork::LiquidTestnet) {
                return Err(Error::Generic(
                    "AMP2 methods are not available for this network".into(),
                ));
            }
            let amp2 = Amp2::new_testnet();
            let pset = PartiallySignedTransaction::from_str(&r.pset).map_err(|e| e.to_string())?;
            let pset = amp2.blocking_cosign(&pset)?.pset.to_string();
            Response::result(
                request.id,
                serde_json::to_value(response::Amp2Cosign { pset })?,
            )
        }
    };
    Ok(response)
}

fn scan(state: &Arc<Mutex<State>>) -> Result<(), Error> {
    let required_scan_loops = {
        let mut s = state.lock()?;
        s.interrupt_wait = true;
        // We want to wait for an _entire_ scan loop to be completed.
        // So if we are scanning, wait for an additional scan loop.
        let is_scanning = s.scan_loops_completed != s.scan_loops_started;
        s.scan_loops_completed + is_scanning as u32
    };
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let current_scan_loops = state.lock()?.scan_loops_completed;
        if current_scan_loops > required_scan_loops {
            break;
        }
        // TODO: fail if waited too much
    }
    Ok(())
}

fn unvalidated_addressee(a: request::UnvalidatedAddressee) -> lwk_wollet::UnvalidatedRecipient {
    lwk_wollet::UnvalidatedRecipient {
        satoshi: a.satoshi,
        address: a.address,
        asset: a.asset,
    }
}

fn signer_response_from(name: &str, signer: &AppSigner) -> Result<response::Signer, Error> {
    Ok(response::Signer {
        name: name.to_string(),
        fingerprint: signer.fingerprint()?.to_string(),
    })
}

fn signer_details(name: &str, signer: &AppSigner) -> Result<response::SignerDetails, Error> {
    Ok(response::SignerDetails {
        name: name.to_string(),
        id: signer.id()?.map(|i| i.to_string()),
        fingerprint: signer.fingerprint()?.to_string(),
        xpub: signer.xpub()?.map(|x| x.to_string()),
        mnemonic: signer.mnemonic(),
        type_: signer.type_(),
    })
}

fn convert_utxo(u: &lwk_wollet::WalletTxOut) -> response::Utxo {
    response::Utxo {
        txid: u.outpoint.txid.to_string(),
        vout: u.outpoint.vout,
        height: u.height,
        script_pubkey: u.script_pubkey.to_hex(),
        address: u.address.to_string(),
        asset: u.unblinded.asset.to_string(),
        value: u.unblinded.value,
    }
}

fn convert_tx(
    tx: &lwk_wollet::WalletTx,
    explorer_url: &str,
    memos: &HashMap<Txid, String>,
) -> response::Tx {
    let unblinded_url = tx.unblinded_url(explorer_url);
    let memo = memos.get(&tx.txid).cloned().unwrap_or_default();
    response::Tx {
        txid: tx.txid.to_string(),
        height: tx.height,
        balance: tx
            .balance
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect(),
        fee: tx.fee,
        timestamp: tx.timestamp,
        type_: tx.type_.clone(),
        unblinded_url,
        memo,
    }
}

fn amp2userkey(signer: &AnySigner) -> Result<String, Error> {
    let bip = lwk_common::Bip::Bip87;
    let is_mainnet = false;
    Ok(signer.keyorigin_xpub(bip, is_mainnet)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppAsset;
    use lwk_wollet::elements::pset::{elip100::PSET_HWW_PREFIX, PartiallySignedTransaction};
    use lwk_wollet::elements::AssetId;
    use lwk_wollet::{Contract, RegistryAssetData};
    use std::collections::HashMap;
    use std::net::TcpListener;
    use std::str::FromStr;

    fn app_random_port() -> App {
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let tempdir = tempfile::tempdir().unwrap();
        let mut config = Config::default_testnet(tempdir.path().to_path_buf());
        config.addr = addr;
        let mut app = App::new(config).unwrap();
        app.run().unwrap();
        app
    }

    #[test]
    fn version() {
        let mut app = app_random_port();
        let addr = app.addr();
        let url = addr.to_string();

        let client = jsonrpc::Client::simple_http(&url, None, None).unwrap();
        let request = client.build_request("version", None);
        let response = client.send_request(request).unwrap();

        let result = response.result.unwrap().to_string();
        let actual: response::Version = serde_json::from_str(&result).unwrap();
        assert_eq!(actual.version, consts::APP_VERSION);

        app.stop().unwrap();
        app.join_threads().unwrap();
    }

    #[test]
    fn add_contracts_embeds_valid_metadata() {
        // Prepare and validate contract
        let contract_json = r#"{"entity":{"domain":"example.com"},"issuer_pubkey":"020202020202020202020202020202020202020202020202020202020202020202","name":"MyCoin","precision":0,"ticker":"MYCO","version":0}"#;
        let contract_value = serde_json::Value::from_str(contract_json).unwrap();
        let contract = Contract::from_value(&contract_value).unwrap();
        contract.validate().unwrap();
        let contract_serialized = serde_json::to_string(&contract).unwrap();
        assert_eq!(contract_serialized, contract_json);

        // Load PSET
        let pset_str = include_str!("../test_data/issuance_pset.base64");
        let mut pset = PartiallySignedTransaction::from_str(pset_str).unwrap();

        // Remove asset metadata preserving initial number of proprietary keys
        let n_proprietary_keys = pset.global.proprietary.len();
        pset.global
            .proprietary
            .retain(|key, _| !key.prefix.starts_with(PSET_HWW_PREFIX));
        assert!(pset
            .global
            .proprietary
            .keys()
            .all(|key| !key.prefix.starts_with(PSET_HWW_PREFIX)));
        let removed = n_proprietary_keys - pset.global.proprietary.len();
        assert_eq!(
            removed, 2,
            "Expected to remove 2 proprietary keys with HWW prefix"
        );

        // Extract transaction
        let tx = pset.extract_tx().unwrap();

        // Prepare assets map
        let mut assets_map: HashMap<AssetId, AppAsset> = HashMap::new();
        let asset_id =
            AssetId::from_str("25e85efe02e5010a880ddb7c936e82896cd7fc493d2a5bc4422e8ec26100b00d")
                .unwrap();
        let asset_data = RegistryAssetData::new(asset_id, tx.clone(), contract.clone())
            .expect("valid registry data");
        let token_id = asset_data.reissuance_token();
        assets_map.insert(asset_id, AppAsset::RegistryAsset(asset_data.clone()));
        assets_map.insert(token_id, AppAsset::ReissuanceToken(asset_data.clone()));

        // TODO: check with a test vector, this is the value generated once the entropy fn has been introduced
        assert_eq!(
            asset_data.entropy().unwrap(),
            [
                246, 117, 58, 147, 49, 238, 254, 149, 30, 173, 173, 99, 251, 157, 220, 73, 16, 39,
                72, 67, 153, 90, 118, 180, 236, 207, 166, 53, 173, 215, 201, 113
            ],
        );

        // Add contracts
        add_contracts(&mut pset, [&asset_data].into_iter());

        // Ensure the asset metadata records are fully re-created
        assert_eq!(
            pset.global.proprietary.len(),
            n_proprietary_keys,
            "Contract metadata was not fully restored"
        );

        // Assert asset metadata is present and valid
        let asset_meta = pset.get_asset_metadata(asset_id).unwrap().unwrap();
        assert_eq!(
            asset_meta.contract(),
            contract_json,
            "Invalid contract in asset metadata"
        );
        assert_eq!(
            asset_meta.issuance_prevout(),
            asset_data.issuance_prevout(),
            "Invalid issuance prevout in asset metadata"
        );

        // Assert token metadata is present and valid
        let token_meta = pset.get_token_metadata(token_id).unwrap().unwrap();
        assert_eq!(
            token_meta.asset_id(),
            &asset_id,
            "Invalid asset tag in reissuance token metadata"
        );
        assert!(
            !token_meta.issuance_blinded(),
            "Invalid issuance blinded flag in reissuance token metadata"
        );
    }
}
