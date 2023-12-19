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
//! All the requests and responses data model are in the [`rpc_model`] crate.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use common::{
    keyorigin_xpub_from_str, multisig_desc, singlesig_desc, InvalidBipVariant,
    InvalidBlindingKeyVariant, InvalidMultisigVariant, InvalidSinglesigVariant, Signer,
};
use jade::mutex_jade::{first_port, MutexJade};
use serde_json::Value;
use signer::{AnySigner, SwSigner};
use state::id_to_fingerprint;
use tiny_jrpc::{tiny_http, JsonRpcServer, Request, Response};
use wollet::bitcoin::bip32::Fingerprint;
use wollet::bitcoin::hash_types::XpubIdentifier;
use wollet::elements::hex::{FromHex, ToHex};
use wollet::elements::pset::PartiallySignedTransaction;
use wollet::elements::{AssetId, TxOutSecrets};
use wollet::elements_miniscript::descriptor::{Descriptor, DescriptorType, WshInner};
use wollet::elements_miniscript::miniscript::decode::Terminal;
use wollet::Wollet;

use crate::method::Method;
use crate::state::{AppAsset, AppSigner, State};
use rpc_model::{request, response};

pub use client::Client;
pub use config::Config;
pub use error::Error;
pub use tiny_jrpc::RpcError;

mod client;
mod config;
pub mod consts;
mod error;
pub mod method;
mod state;

pub struct App {
    rpc: Option<JsonRpcServer>,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Result<App, Error> {
        tracing::info!("Creating new app with config: {:?}", config);

        Ok(App { rpc: None, config })
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
            do_persist: false,
        };
        state.insert_policy_asset();
        let state = Arc::new(Mutex::new(state));
        let server = tiny_http::Server::http(self.config.addr)?;

        let rpc = tiny_jrpc::JsonRpcServer::new(
            server,
            tiny_jrpc::Config::default(),
            state.clone(),
            method_handler,
        );

        let path = self.config.state_path()?;
        match std::fs::read_to_string(&path) {
            Ok(string) => {
                tracing::info!(
                    "Loading previous state, {} elements",
                    string.lines().count()
                );

                let client = self.client()?;

                for line in string.lines() {
                    let r: Request = serde_json::from_str(line)?;
                    let method: Method = r.method.parse()?;
                    let _value: Value = client.make_request(method, r.params)?;
                }
            }
            Err(_) => {
                tracing::info!("There is no previous state at {path:?}");
            }
        }
        state.lock().map_err(|e| e.to_string())?.do_persist = true;

        self.rpc = Some(rpc);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), Error> {
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
        Ok(())
    }

    pub fn client(&self) -> Result<Client, Error> {
        Client::new(self.config.addr)
    }
}

fn method_handler(
    request: Request,
    state: Arc<Mutex<State>>,
) -> Result<Response, tiny_jrpc::Error> {
    Ok(inner_method_handler(request, state)?)
}

fn inner_method_handler(request: Request, state: Arc<Mutex<State>>) -> Result<Response, Error> {
    tracing::debug!(
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
        Method::GenerateSigner => {
            let (_signer, mnemonic) = SwSigner::random()?;
            Response::result(
                request.id,
                serde_json::to_value(response::GenerateSigner {
                    mnemonic: mnemonic.to_string(),
                })?,
            )
        }
        Method::Version => Response::result(
            request.id,
            serde_json::to_value(response::Version {
                version: consts::APP_VERSION.into(),
            })?,
        ),
        Method::LoadWallet => {
            let r: request::LoadWallet = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            // TODO recognize different name same descriptor?
            let wollet = Wollet::new(
                s.config.network.clone(),
                &s.config.electrum_url,
                s.config.tls,
                s.config.validate_domain,
                &s.config.datadir.display().to_string(),
                &r.descriptor,
            )?;
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
        Method::UnloadWallet => {
            let r: request::UnloadWallet = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let removed = s.wollets.remove(&r.name)?;
            s.persist_all()?;

            Response::result(
                request.id,
                serde_json::to_value(response::UnloadWallet {
                    unloaded: response::Wallet {
                        name: r.name,
                        descriptor: removed.descriptor().to_string(),
                    },
                })?,
            )
        }
        Method::ListWallets => {
            let s = state.lock()?;
            let wallets = s
                .wollets
                .iter()
                .map(|(name, wollet)| response::Wallet {
                    descriptor: wollet.descriptor().to_string(),
                    name: name.clone(),
                })
                .collect();
            let r = response::ListWallets { wallets };
            Response::result(request.id, serde_json::to_value(r)?)
        }
        Method::SignerLoadSoftware => {
            let r: request::SignerLoadSoftware = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let mnemonic = r.mnemonic;
            let signer = AppSigner::AvailableSigner(AnySigner::Software(SwSigner::new(&mnemonic)?));
            let resp: response::Signer = signer_response_from(&r.name, &signer)?;
            s.signers.insert(&r.name, signer)?;
            s.persist(&request)?;
            Response::result(request.id, serde_json::to_value(resp)?)
        }
        Method::SignerLoadJade => {
            let r: request::SignerLoadJade = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let id = XpubIdentifier::from_str(&r.id).map_err(|e| e.to_string())?; // TODO remove map_err
            let signer = match r.emulator {
                Some(socket) => {
                    // The emulator is meant to be used only in testing, we don't aim to handle connection/disconnection
                    let jade = MutexJade::from_socket(socket, s.config.jade_network())?;
                    AppSigner::AvailableSigner(AnySigner::Jade(jade, id))
                }
                None => AppSigner::JadeId(id, s.config.jade_network()),
            };
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
            let signer = AppSigner::ExternalSigner(fingerprint);
            let resp: response::Signer = signer_response_from(&r.name, &signer)?;
            s.signers.insert(&r.name, signer)?;
            s.persist(&request)?;
            Response::result(request.id, serde_json::to_value(resp)?)
        }
        Method::UnloadSigner => {
            let r: request::UnloadSigner = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let removed = s.signers.remove(&r.name)?;
            let signer: response::Signer = signer_response_from(&r.name, &removed)?;
            s.persist_all()?;
            Response::result(
                request.id,
                serde_json::to_value(response::UnloadSigner { unloaded: signer })?,
            )
        }
        Method::ListSigners => {
            let s = state.lock()?;
            let signers: Result<Vec<_>, _> = s
                .signers
                .iter()
                .map(|(name, signer)| signer_response_from(name, signer))
                .collect();
            let mut signers = signers?;
            signers.sort();
            let r = response::ListSigners { signers };
            Response::result(request.id, serde_json::to_value(r)?)
        }
        Method::Address => {
            let r: request::Address = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            wollet.sync_txs()?; // To update the last unused index
            let addr = wollet.address(r.index)?;
            Response::result(
                request.id,
                serde_json::to_value(response::Address {
                    address: addr.address().to_string(),
                    index: addr.index(),
                })?,
            )
        }
        Method::Balance => {
            let r: request::Balance = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            wollet.sync_txs()?;
            let mut balance = wollet
                .balance()?
                .into_iter()
                .map(|(k, v)| (k.to_string(), v as i64))
                .collect();
            if r.with_tickers {
                balance = s.replace_id_with_ticker(balance);
            }
            Response::result(
                request.id,
                serde_json::to_value(response::Balance { balance })?,
            )
        }
        Method::SendMany => {
            let r: request::Send = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            wollet.sync_txs()?;
            let mut tx = wollet.send_many(
                r.addressees
                    .into_iter()
                    .map(unvalidated_addressee)
                    .collect(),
                r.fee_rate,
            )?;
            add_contracts(&mut tx, s.assets.iter());
            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: tx.to_string(),
                })?,
            )
        }
        Method::SinglesigDescriptor => {
            let r: request::SinglesigDescriptor = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let signer = s.signers.get_available(&r.name)?;

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
                serde_json::to_value(response::SinglesigDescriptor { descriptor })?,
            )
        }
        Method::MultisigDescriptor => {
            let r: request::MultisigDescriptor = serde_json::from_value(params)?;

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
                serde_json::to_value(response::MultisigDescriptor { descriptor })?,
            )
        }
        Method::Xpub => {
            let r: request::Xpub = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let signer = s.signers.get_available(&r.name)?;

            let bip = r
                .xpub_kind
                .parse()
                .map_err(|e: InvalidBipVariant| e.to_string())?;

            let keyorigin_xpub = signer.keyorigin_xpub(bip)?;
            Response::result(
                request.id,
                serde_json::to_value(response::Xpub { keyorigin_xpub })?,
            )
        }
        Method::Sign => {
            let r: request::Sign = serde_json::from_value(params)?;
            let mut s = state.lock()?;

            let signer = s.signers.get_available(&r.name)?;

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
        Method::Broadcast => {
            let r: request::Broadcast = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            let mut pset =
                PartiallySignedTransaction::from_str(&r.pset).map_err(|e| e.to_string())?;
            let tx = wollet.finalize(&mut pset)?;

            if !r.dry_run {
                wollet.broadcast(&tx)?;
            }

            Response::result(
                request.id,
                serde_json::to_value(response::Broadcast {
                    txid: tx.txid().to_string(),
                })?,
            )
        }
        Method::WalletDetails => {
            let r: request::WalletDetails = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;

            let type_ = match wollet.descriptor().descriptor.desc_type() {
                DescriptorType::Wpkh => response::WalletType::Wpkh,
                DescriptorType::ShWpkh => response::WalletType::ShWpkh,
                _ => match &wollet.descriptor().descriptor {
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
                    response::SignerDetails {
                        name,
                        fingerprint: fingerprint.to_string(),
                    }
                })
                .collect();

            Response::result(
                request.id,
                serde_json::to_value(response::WalletDetails {
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
                .map(|f| response::SignerDetails {
                    name: s.signers.name_from_fingerprint(f, &mut warnings),
                    fingerprint: f.to_string(),
                })
                .collect();
            let missing_signatures_from = details
                .fingerprints_missing()
                .iter()
                .map(|f| response::SignerDetails {
                    name: s.signers.name_from_fingerprint(f, &mut warnings),
                    fingerprint: f.to_string(),
                })
                .collect();
            let mut balance: HashMap<String, i64> = details
                .balance
                .balances
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
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
            wollet.sync_txs()?;
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
            let wollet = s.wollets.get_mut(&r.name)?;
            wollet.sync_txs()?;
            let mut txs: Vec<response::Tx> = wollet
                .transactions()?
                .iter()
                .map(|tx| convert_tx(tx, &explorer_url))
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
        Method::Issue => {
            let r: request::Issue = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            wollet.sync_txs()?;
            let tx = wollet.issue_asset(
                r.satoshi_asset,
                r.address_asset.as_deref().unwrap_or(""),
                r.satoshi_token,
                r.address_token.as_deref().unwrap_or(""),
                r.contract.as_deref().unwrap_or(""),
                r.fee_rate,
            )?;
            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: tx.to_string(),
                })?,
            )
        }
        Method::Reissue => {
            let r: request::Reissue = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let wollet = s.wollets.get_mut(&r.name)?;
            wollet.sync_txs()?;
            let mut pset = wollet.reissue_asset(
                &r.asset,
                r.satoshi_asset,
                r.address_asset.as_deref().unwrap_or(""),
                r.fee_rate,
            )?;
            add_contracts(&mut pset, s.assets.iter());
            Response::result(
                request.id,
                serde_json::to_value(response::Pset {
                    pset: pset.to_string(),
                })?,
            )
        }
        Method::Contract => {
            let r: request::Contract = serde_json::from_value(params)?;
            let c = wollet::Contract {
                entity: wollet::Entity::Domain(r.domain),
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
            let asset_id = wollet::elements::AssetId::from_str(&r.asset_id)
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
        Method::ListAssets => {
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
            let r = response::ListAssets { assets };
            Response::result(request.id, serde_json::to_value(r)?)
        }
        Method::AssetInsert => {
            let r: request::AssetInsert = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let asset_id = wollet::elements::AssetId::from_str(&r.asset_id)
                .map_err(|e| Error::Generic(e.to_string()))?;
            let prev_txid = wollet::elements::Txid::from_str(&r.prev_txid)
                .map_err(|e| Error::Generic(e.to_string()))?;
            let contract = serde_json::Value::from_str(&r.contract)?;
            let contract = wollet::Contract::from_value(&contract)?;
            s.insert_asset(
                asset_id,
                prev_txid,
                r.prev_vout,
                contract,
                r.is_confidential,
            )?;
            s.persist(&request)?;
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::AssetRemove => {
            let r: request::AssetRemove = serde_json::from_value(params)?;
            let mut s = state.lock()?;
            let asset_id = wollet::elements::AssetId::from_str(&r.asset_id)
                .map_err(|e| Error::Generic(e.to_string()))?;
            s.remove_asset(&asset_id)?;
            s.persist_all()?;
            Response::result(request.id, serde_json::to_value(response::Empty {})?)
        }
        Method::SignerJadeId => {
            let r: request::SignerJadeId = serde_json::from_value(params)?;

            let network = {
                let s = state.lock()?;
                s.config.jade_network()
            };

            let jade = match r.emulator {
                Some(emulator) => MutexJade::from_socket(emulator, network)?,
                None => MutexJade::from_serial(network, &first_port()?)?,
            };
            jade.unlock()?;
            let identifier = jade.identifier()?.to_string();
            Response::result(
                request.id,
                serde_json::to_value(response::JadeId { identifier })?,
            )
        }
        Method::Stop => {
            return Err(Error::Stop);
        }
    };
    Ok(response)
}

fn unvalidated_addressee(a: request::UnvalidatedAddressee) -> wollet::UnvalidatedAddressee {
    wollet::UnvalidatedAddressee {
        satoshi: a.satoshi,
        address: a.address,
        asset: a.asset,
    }
}

fn signer_response_from(
    name: &str,
    signer: &AppSigner,
) -> Result<response::Signer, signer::SignerError> {
    let (fingerprint, id, xpub) = match signer {
        AppSigner::AvailableSigner(signer) => (
            signer.fingerprint()?,
            Some(signer.identifier()?),
            Some(signer.xpub()?),
        ),
        AppSigner::ExternalSigner(fingerprint) => (*fingerprint, None, None),
        AppSigner::JadeId(id, _) => (id_to_fingerprint(id), Some(*id), None),
    };

    Ok(response::Signer {
        name: name.to_string(),
        id: id.map(|i| i.to_string()),
        fingerprint: fingerprint.to_string(),
        xpub: xpub.map(|x| x.to_string()),
    })
}

fn add_contracts<'a>(
    pset: &mut PartiallySignedTransaction,
    assets: impl Iterator<Item = (&'a AssetId, &'a AppAsset)>,
) {
    let assets_in_pset: HashSet<_> = pset.outputs().iter().filter_map(|o| o.asset).collect();
    for (_, asset) in assets {
        if let AppAsset::RegistryAsset(_) = asset {
            // Policy asset and reissuance tokens do not require the contract
            let asset_id = asset.asset_id();
            if assets_in_pset.contains(&asset_id) {
                if let Some(metadata) = asset.asset_metadata() {
                    pset.add_asset_metadata(asset_id, &metadata);
                }
            }
        }
    }
}

fn convert_utxo(u: &wollet::WalletTxOut) -> response::Utxo {
    response::Utxo {
        txid: u.outpoint.txid.to_string(),
        vout: u.outpoint.vout,
        height: u.height,
        script_pubkey: u.script_pubkey.to_hex(),
        asset: u.unblinded.asset.to_string(),
        value: u.unblinded.value,
    }
}

fn fmt_txoutsecrets(s: &TxOutSecrets) -> String {
    format!("{},{},{},{}", s.value, s.asset, s.value_bf, s.asset_bf)
}

fn fmt_wallet_txouts(txouts: &[Option<wollet::WalletTxOut>]) -> Vec<String> {
    txouts
        .iter()
        .filter_map(|f| f.as_ref())
        .map(|e| fmt_txoutsecrets(&e.unblinded))
        .collect()
}

fn convert_tx(tx: &wollet::WalletTx, explorer_url: &str) -> response::Tx {
    let txid = tx.tx.txid().to_string();
    let mut blinded = fmt_wallet_txouts(&tx.inputs);
    blinded.extend(fmt_wallet_txouts(&tx.outputs));
    let blinded = blinded.join(",");
    let unblinded_url = format!("{explorer_url}tx/{txid}#blinded={blinded}");
    response::Tx {
        txid,
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
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

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
        dbg!(&url);

        let client = jsonrpc::Client::simple_http(&url, None, None).unwrap();
        let request = client.build_request("version", None);
        let response = client.send_request(request).unwrap();

        let result = response.result.unwrap().to_string();
        let actual: response::Version = serde_json::from_str(&result).unwrap();
        assert_eq!(actual.version, consts::APP_VERSION);

        app.stop().unwrap();
        app.join_threads().unwrap();
    }
}
