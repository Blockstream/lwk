#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Contains the data model to communicate with the RPC server divided
//! in [`request`]s and [`response`]s.
//!

pub mod request;
pub mod response;
