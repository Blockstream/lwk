//! Asyncronous clients to fetch data from the Blockchain. Suitable to be used in WASM environments like in the browser.

#[cfg(feature = "esplora")]
mod esplora;

#[cfg(feature = "esplora")]
pub use crate::clients::{EsploraClientBuilder, WaterfallsClientBuilder};

#[cfg(all(feature = "esplora", not(target_arch = "wasm32")))]
pub use esplora::WaterfallsSubscription;
#[cfg(feature = "esplora")]
pub use esplora::{EsploraClient, LastUsedIndexResponse, WaterfallsClient};

pub use crate::async_util::{async_now, async_sleep};
