//! Contains the data model to communicate with the RPC server divided
//! in [`request`]s and [`response`]s.
//!

use schemars::JsonSchema;

pub mod request;
pub mod response;

#[derive(JsonSchema)]
pub struct Empty {}
