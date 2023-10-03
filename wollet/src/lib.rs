mod config;
mod domain;
mod error;
mod model;
mod pset_create;
mod pset_details;
mod registry;
mod store;
mod sync;
mod util;
mod wallet;

pub use crate::config::ElementsNetwork;
pub use crate::error::Error;
pub use crate::model::{AddressResult, IssuanceDetails, UnvalidatedAddressee, WalletTxOut};
pub use crate::pset_details::*;
pub use crate::util::EC;
pub use crate::wallet::ElectrumWallet;

pub use elements_miniscript::elements;
pub use elements_miniscript::elements::bitcoin::{self, hashes, secp256k1};
