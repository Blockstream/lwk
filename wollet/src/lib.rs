mod config;
mod domain;
mod error;
mod model;
mod pset_create;
mod registry;
mod store;
mod sync;
mod util;
mod wallet;
mod wollet_desc;

pub use crate::config::ElementsNetwork;
pub use crate::error::Error;
pub use crate::model::{AddressResult, IssuanceDetails, UnvalidatedAddressee, WalletTxOut};
pub use crate::util::EC;
pub use crate::wallet::ElectrumWallet;
pub use crate::wollet_desc::WolletDescriptor;

pub use elements_miniscript::elements;
pub use elements_miniscript::elements::bitcoin::{self, hashes, secp256k1};
