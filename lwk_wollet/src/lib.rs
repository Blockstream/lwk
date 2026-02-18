#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

//! LWK is a collection of libraries for Liquid wallets.
//! `lwk_wollet` is the library for Watch-Only Wallets, the `wollet` spelling is not a typo but highlights the fact it is Watch-Only.
//!
//! A [`Wollet`] is defined by a [CT descriptor](https://github.com/ElementsProject/ELIPs/blob/main/elip-0150.mediawiki),
//! which consists in a Bitcoin descriptor plus the descriptor blinding key. More precisely a subset of descriptors are supported, everything parsed by [`WolletDescriptor`].
//! Every method on the `Wollet` will operate on local data, without network calls. The wallet data is updated via the [`Wollet::apply_update()`] method.
//!
//! With a wallet you can:
//! * Generate addresses via [`Wollet::address()`].
//! * Pass it to a blockchain backend ([`ElectrumClient`], [`blocking::EsploraClient`]) to retrieve wallet history via the [`blocking::BlockchainBackend::full_scan()`] trait.
//!   Or asyncronously via the [`asyncr::EsploraClient::full_scan()`] method. The convenience method [`full_scan_with_electrum_client()`] is also provided.
//! * Create transactions, inclunding issuances, reissuances and burn via the [`TxBuilder`].
//! * Analyze a partially signed transaction with respect to the wallet via [`Wollet::get_details()`].
//!
//! ## Examples
//!
//! ### Generate an address
//! ```rust
//! # use lwk_wollet::{WolletDescriptor, Wollet, ElementsNetwork};
//! # fn main() -> Result<(), lwk_wollet::Error> {
//! let desc = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
//!
//! // Parse the descriptor and create the watch only wallet
//! let descriptor: WolletDescriptor = desc.parse()?;
//! let mut wollet = Wollet::without_persist(
//!    ElementsNetwork::LiquidTestnet,
//!    descriptor,
//! )?;
//!
//! // Generate the address
//! let addr = wollet.address(None)?;
//! println!("Address: {} (index {})", addr.address(), addr.index());
//! # Ok(())
//! # }
//! ```
//!
//! ### Sync wallet
//! ```rust,no_run
//! # use lwk_wollet::{WolletDescriptor, Wollet, ElementsNetwork, ElectrumClient, ElectrumUrl,
//! full_scan_with_electrum_client};
//! # fn main() -> Result<(), lwk_wollet::Error> {
//! # let desc = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
//! # let descriptor: WolletDescriptor = desc.parse()?;
//! # let mut wollet = Wollet::without_persist(
//! #    ElementsNetwork::LiquidTestnet,
//! #    descriptor,
//! # )?;
//! // Use an Electrum server
//! let electrum_url = ElectrumUrl::new("elements-testnet.blockstream.info:50002", true, true)?;
//! let mut electrum_client = ElectrumClient::new(&electrum_url)?;
//! full_scan_with_electrum_client(&mut wollet, &mut electrum_client)?;
//!
//! // Print a summary of the wallet transactions
//! for tx in wollet.transactions()?.into_iter().rev() {
//!     println!("TXID: {}, balance {:?}", tx.txid, tx.balance);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Create transaction
//! ```rust,no_run
//! # use lwk_wollet::{WolletDescriptor, Wollet, ElementsNetwork, UnvalidatedRecipient};
//! # fn main() -> Result<(), lwk_wollet::Error> {
//! # let desc = "ct(slip77(ab5824f4477b4ebb00a132adfd8eb0b7935cf24f6ac151add5d1913db374ce92),elwpkh([759db348/84'/1'/0']tpubDCRMaF33e44pcJj534LXVhFbHibPbJ5vuLhSSPFAw57kYURv4tzXFL6LSnd78bkjqdmE3USedkbpXJUPA1tdzKfuYSL7PianceqAhwL2UkA/<0;1>/*))#cch6wrnp";
//! # let descriptor: WolletDescriptor = desc.parse()?;
//! # let mut wollet = Wollet::without_persist(
//! #    ElementsNetwork::LiquidTestnet,
//! #    descriptor,
//! # )?;
//! // Create a transaction
//! let recipient = UnvalidatedRecipient {
//!     satoshi: 1000,
//!     address: "tlq1qqgpjea0jcel4tqeln5kyxlrgqx2eh4vw67ecswm54476mddy3n0klrlmty5gn0wsdw4045rtl2y2wdtr4rdu6v93zds6zn8xd".to_string(),
//!     asset: "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49".to_string(),
//! };
//! let pset = wollet
//!     .tx_builder()
//!     .add_unvalidated_recipient(&recipient)?
//!     .finish()?;
//!
//! // Then pass the PSET to the signer(s) for them to sign.
//! # Ok(())
//! # }
//! ```

#[cfg(feature = "amp0")]
pub mod amp0;
#[cfg(feature = "amp2")]
pub mod amp2;

mod cache;
pub mod clients;
mod contract;
mod descriptor;
mod domain;
mod error;
mod liquidex;
mod model;
mod network;
pub mod pegin;

#[cfg(feature = "prices")]
mod pos;

mod pset_create;
#[cfg(feature = "registry")]
pub mod registry;
mod tx_builder;
mod update;
mod util;
#[cfg(feature = "amp0")]
mod wamp;
mod wollet;

#[cfg(feature = "prices")]
pub mod prices;

pub use crate::clients::electrum_url::{ElectrumUrl, UrlError};
pub use crate::clients::{Capability, History};
pub use crate::contract::{asset_ids, issuance_ids, Contract, Entity};
pub use crate::descriptor::{Chain, WolletDescriptor};
pub use crate::error::Error;
pub use crate::liquidex::{AssetAmount, LiquidexProposal, Unvalidated, Validated};
pub use crate::model::{
    AddressResult, ExternalUtxo, IssuanceDetails, Recipient, UnvalidatedRecipient, WalletTx,
    WalletTxOut,
};
pub use crate::network::ElementsNetwork;
pub use crate::pegin::fed_peg_script;
#[cfg(feature = "registry")]
pub use crate::registry::RegistryAssetData;
pub use crate::wollet::DirectoryIdHash;

// Re-export store types from lwk_common
pub use crate::tx_builder::{TxBuilder, WolletTxBuilder};
pub use crate::update::{DownloadTxResult, Update};
pub use crate::util::EC;
pub use crate::wollet::{Tip, Wollet, WolletBuilder};
pub use lwk_common::{
    BoxError, DynStore, EncryptedStore, EncryptedStoreError, FakeStore, FileStore, MemoryStore,
    Store,
};

#[cfg(feature = "prices")]
pub use crate::prices::{
    CurrencyCode, ExchangeRate, ExchangeRates, PricesFetcher, PricesFetcherBuilder,
};

#[cfg(feature = "prices")]
pub use crate::pos::PosConfig;

#[cfg(feature = "electrum")]
pub use crate::wollet::full_scan_to_index_with_electrum_client;
#[cfg(feature = "electrum")]
pub use crate::wollet::full_scan_with_electrum_client;
#[cfg(feature = "electrum")]
pub use clients::blocking::electrum_client::{ElectrumClient, ElectrumOptions};

#[cfg(feature = "esplora")]
pub use age;

pub use crate::clients::asyncr;

#[cfg(feature = "electrum")]
pub use crate::clients::blocking;

#[cfg(feature = "elements_rpc")]
pub use clients::blocking::ElementsRpcClient;

#[cfg(feature = "elements_rpc")]
pub use bitcoincore_rpc;

pub use elements_miniscript;
pub use elements_miniscript::elements;
pub use elements_miniscript::elements::bitcoin::{self, hashes, secp256k1};

/// The blinding public key used for confidential addresses.
pub type BlindingPublicKey = elements::secp256k1_zkp::PublicKey;

pub(crate) mod hex {
    use elements::{hashes::hex::DisplayHex, hex::Error, hex::FromHex};

    pub fn _encode(data: &[u8]) -> String {
        data.to_lower_hex_string()
    }

    pub fn _decode(data: &str) -> Result<Vec<u8>, Error> {
        Vec::<u8>::from_hex(data)
    }
}
