#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! # Wollet
//!
//! An elements and liquid Watch-Only Wallet defined by a
//! [CT descriptor](https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki).
//!
//! For an entry point see [`Wollet::new()`]

mod config;
mod descriptor;
mod domain;
mod error;
mod model;
mod pset_create;
mod registry;
mod store;
mod sync;
mod util;
mod wollet;

pub use crate::config::ElementsNetwork;
pub use crate::descriptor::WolletDescriptor;
pub use crate::error::Error;
pub use crate::model::{
    AddressResult, IssuanceDetails, UnvalidatedAddressee, WalletTx, WalletTxOut,
};
pub use crate::registry::{asset_ids, issuance_ids, Contract, Entity};
pub use crate::util::EC;
pub use crate::wollet::Wollet;

pub use elements_miniscript;
pub use elements_miniscript::elements;
pub use elements_miniscript::elements::bitcoin::{self, hashes, secp256k1};
