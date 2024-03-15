#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! # Wollet
//!
//! An elements and liquid Watch-Only Wallet defined by a
//! [CT descriptor](https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki).
//!
//! For an entry point see [`Wollet::new()`]

mod clients;
mod config;
mod descriptor;
mod domain;
mod error;
mod model;
mod persister;
mod pset_create;
mod registry;
mod store;
mod tx_builder;
mod update;
mod util;
mod wollet;

pub use crate::clients::BlockchainBackend;
pub use crate::config::ElementsNetwork;
pub use crate::descriptor::{Chain, WolletDescriptor};
pub use crate::error::Error;
pub use crate::model::{
    AddressResult, Addressee, IssuanceDetails, UnvalidatedAddressee, WalletTx, WalletTxOut,
};
pub use crate::persister::{FsPersister, NoPersist, PersistError, Persister};
pub use crate::registry::{asset_ids, issuance_ids, Contract, Entity};
pub use crate::tx_builder::TxBuilder;
pub use crate::update::Update;
pub use crate::util::EC;
pub use crate::wollet::Wollet;

#[cfg(feature = "electrum")]
pub use crate::wollet::full_scan_with_electrum_client;
#[cfg(feature = "electrum")]
pub use clients::electrum_client::{ElectrumClient, ElectrumUrl};

#[cfg(feature = "esplora")]
pub use clients::esplora_client::EsploraClient;

#[cfg(feature = "esplora_wasm")]
pub use clients::esplora_wasm_client::EsploraWasmClient;

#[cfg(feature = "esplora_wasm")]
pub use clients::esplora_wasm_client::async_sleep;

pub use elements_miniscript;
pub use elements_miniscript::elements;
pub use elements_miniscript::elements::bitcoin::{self, hashes, secp256k1};
