#![doc = include_str!("../README.md")]

pub mod asset_data;
pub mod contract;
pub mod domain;
pub mod error;
pub mod util;

#[cfg(feature = "client")]
pub mod registry;

#[cfg(feature = "client")]
pub use registry::{
    Registry, RegistryCache, RegistryData, RegistryPost, TxFetcher, TxFetcherAsync,
};

pub use asset_data::{add_contracts, RegistryAssetData};
pub use contract::{asset_ids, issuance_ids, Contract, Entity};
