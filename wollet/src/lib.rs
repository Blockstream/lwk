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
pub use crate::model::{AddressResult, IssuanceDetails, UnvalidatedAddressee, WalletTxOut};
pub use crate::util::EC;
pub use crate::wollet::Wollet;

pub use elements_miniscript::elements;
pub use elements_miniscript::elements::bitcoin::{self, hashes, secp256k1};
