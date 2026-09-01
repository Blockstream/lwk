#![doc = include_str!("../README.md")]

pub mod contract;
pub mod domain;
pub mod error;
pub mod util;

#[cfg(feature = "client")]
pub mod registry;

#[cfg(feature = "client")]
pub use registry::{
    add_contracts, Registry, RegistryAssetData, RegistryCache, RegistryData, RegistryPost,
    TxFetcher, TxFetcherAsync,
};

pub use contract::{asset_ids, issuance_ids, Contract, Entity};
