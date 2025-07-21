#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc = include_str!("../README.md")]

mod amp2;
pub mod blockdata;
mod chain;
mod contract;
mod desc;
mod electrum_client;
mod error;
mod esplora_client;
mod liquidex;
mod mnemonic;
mod network;
mod persister;
mod precision;
mod pset;
mod pset_details;
mod signer;
mod test_env;
mod tx_builder;
pub mod types;
mod update;
mod wollet;

pub use blockdata::address::Address;
pub use blockdata::address_result::AddressResult;
pub use blockdata::block_header::BlockHeader;
pub use blockdata::external_utxo::ExternalUtxo;
pub use blockdata::out_point::OutPoint;
pub use blockdata::script::Script;
pub use blockdata::transaction::Transaction;
pub use blockdata::tx_in::TxIn;
pub use blockdata::tx_out::TxOut;
pub use blockdata::tx_out_secrets::TxOutSecrets;
pub use blockdata::txid::Txid;
pub use blockdata::wallet_tx::WalletTx;
pub use blockdata::wallet_tx_out::WalletTxOut;

pub use crate::contract::Contract;
pub use crate::signer::{Bip, Signer};
pub use crate::wollet::Wollet;
pub use chain::Chain;
pub use desc::WolletDescriptor;
pub use electrum_client::ElectrumClient;
pub use error::LwkError;
pub use esplora_client::EsploraClient;
pub use liquidex::{AssetAmount, UnvalidatedLiquidexProposal, ValidatedLiquidexProposal};
pub use mnemonic::Mnemonic;
pub use network::Network;
pub use persister::{ForeignPersister, ForeignPersisterLink};
pub use precision::Precision;
pub use pset::{Pset, PsetInput};
pub use pset_details::{Issuance, PsetDetails};
pub use test_env::LwkTestEnv;
pub use tx_builder::TxBuilder;
pub use update::Update;

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {}
