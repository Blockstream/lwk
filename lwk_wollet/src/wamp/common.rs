/// Copied from https://github.com/sideswap-io/sideswap_rust/ with minor modifications
/// Copied from https://github.com/elast0ny/wamp_async with minor modifications
/// [MIT license](http://opensource.org/licenses/MIT)
///
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

/// uri: a string URI as defined in URIs
pub type WampUri = String;

/// id: an integer ID as defined in IDs
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub struct WampId(u64);

impl fmt::Display for WampId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

static REQUEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl WampId {
    pub fn generate() -> Self {
        WampId(REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1)
    }
}

/// integer: a non-negative integer
pub type WampInteger = usize;
/// string: a Unicode string, including the empty string
pub type WampString = String;
/// bool: a boolean value (true or false)
pub type WampBool = bool;
/// dict: a dictionary (map) where keys MUST be strings
pub type WampDict = HashMap<String, Arg>;
/// list: a list (array) where items can be of any type
pub type WampList = Vec<Arg>;
/// Arbitrary values supported by the serialization format in the payload
pub type WampPayloadValue = rmpv::Value;
/// Unnamed WAMP argument list
pub type WampArgs = Vec<WampPayloadValue>;
/// Named WAMP argument map
pub type WampKwArgs = serde_json::Map<String, serde_json::Value>;

/// Generic enum that can hold any concrete WAMP value
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Arg {
    /// uri: a string URI as defined in URIs
    Uri(WampUri),
    /// id: an integer ID as defined in IDs
    Id(WampId),
    /// integer: a non-negative integer
    Integer(WampInteger),
    /// string: a Unicode string, including the empty string
    String(WampString),
    /// bool: a boolean value (true or false)
    Bool(WampBool),
    /// dict: a dictionary (map) where keys MUST be strings
    Dict(WampDict),
    /// list: a list (array) where items can be again any of this enumeration
    List(WampList),
    None,
}

#[allow(unused)]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
/// All roles a client can be
pub enum ClientRole {
    /// Client can call RPC endpoints
    Caller,
    /// Client can register RPC endpoints
    Callee,
    /// Client can publish events to topics
    Publisher,
    /// Client can register for events on topics
    Subscriber,
}

impl ClientRole {
    /// Returns the string repesentation of the role
    pub fn to_str(&self) -> &'static str {
        match self {
            ClientRole::Caller => "caller",
            ClientRole::Callee => "callee",
            ClientRole::Publisher => "publisher",
            ClientRole::Subscriber => "subscriber",
        }
    }
}
