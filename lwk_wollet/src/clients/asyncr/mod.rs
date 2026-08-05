//! Asyncronous clients to fetch data from the Blockchain. Suitable to be used in WASM environments like in the browser.

#[cfg(feature = "esplora")]
mod esplora;

// Parsing of Esplora's JSON block listing, used to get a block's prevout scripts in a
// handful of requests instead of one per input.
#[cfg(all(feature = "esplora", feature = "silentpayments"))]
mod block_prevouts;

#[cfg(feature = "esplora")]
pub use crate::clients::{EsploraClientBuilder, WaterfallsClientBuilder};

#[cfg(feature = "esplora")]
pub use esplora::{EsploraClient, LastUsedIndexResponse, WaterfallsClient};
#[cfg(all(feature = "esplora", not(target_arch = "wasm32")))]
pub use esplora::{WaterfallsReconnectingSubscription, WaterfallsSubscription};

pub use crate::async_util::{async_now, async_sleep};
