#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

pub mod consts;
pub mod error;
pub mod get_receive_address;
mod network;
pub mod protocol;
pub mod register_multisig;
pub mod sign_liquid_tx;
pub mod sign_pset;
mod sync;

pub use consts::{BAUD_RATE, TIMEOUT};
use elements::bitcoin::bip32::DerivationPath;
pub use error::Error;
pub use network::Network;
pub use sync::Jade;

#[cfg(feature = "serial")]
pub use serialport;

pub type Result<T> = std::result::Result<T, error::Error>;

fn try_parse_response<T>(reader: &[u8]) -> Option<Result<T>>
where
    T: std::fmt::Debug + serde::de::DeserializeOwned,
{
    match serde_cbor::from_reader::<protocol::Response<T>, &[u8]>(reader) {
        Ok(r) => {
            if let Some(result) = r.result {
                tracing::debug!(
                    "\n<---\t{:?}\n\t({} bytes) {}",
                    &result,
                    reader.len(),
                    hex::encode(reader)
                );
                return Some(Ok(result));
            }
            if let Some(error) = r.error {
                return Some(Err(Error::JadeError(error)));
            }
            return Some(Err(Error::JadeNeitherErrorNorResult));
        }

        Err(e) => {
            let res = serde_cbor::from_reader::<serde_cbor::Value, &[u8]>(reader);
            if let Ok(value) = res {
                // The value returned is a valid CBOR, but our structs doesn't map it correctly
                dbg!(&value);
                return Some(Err(Error::SerdeCbor(e)));
            }
        }
    }
    None
}

pub fn derivation_path_to_vec(path: &DerivationPath) -> Vec<u32> {
    path.into_iter().map(|e| (*e).into()).collect()
}

pub(crate) fn vec_to_derivation_path(path: &[u32]) -> DerivationPath {
    DerivationPath::from_iter(path.iter().cloned().map(Into::into))
}
