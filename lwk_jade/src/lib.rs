#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "asyncr")]
pub mod asyncr;

pub mod consts;
pub mod error;
pub mod get_receive_address;
mod network;
pub mod protocol;
pub mod register_multisig;
pub mod sign_liquid_tx;

#[cfg(feature = "sync")]
mod sync;

pub use consts::{BAUD_RATE, TIMEOUT};
use elements::bitcoin::bip32::DerivationPath;
pub use error::Error;
pub use network::Network;

#[cfg(feature = "sync")]
pub use sync::Jade;

#[cfg(feature = "serial")]
pub use serialport;

pub type Result<T> = std::result::Result<T, error::Error>;

/// Vendor ID and Product ID to filter blockstream JADEs on the serial.
///
/// Note these refer to the usb serial chip not to the JADE itself, so you may have false-positive.
///
/// Note that DYI device may be filtered out by these.
///
/// Taken from reference impl <https://github.com/Blockstream/Jade/blob/f7fc4de8c3662b082c7d41e9354c4ff573f371ff/jadepy/jade_serial.py#L24>
pub const JADE_DEVICE_IDS: [(u16, u16); 4] = [
    (0x10c4, 0xea60),
    (0x1a86, 0x55d4),
    (0x0403, 0x6001),
    (0x1a86, 0x7523),
];

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

pub(crate) fn json_to_cbor(value: &serde_json::Value) -> Result<serde_cbor::Value> {
    // serde_cbor::to_value doesn't exist
    Ok(serde_cbor::from_slice(&serde_cbor::to_vec(&value)?)?)
}

#[cfg(test)]
mod test {
    use crate::json_to_cbor;

    fn cbor_to_json(value: serde_cbor::Value) -> Result<serde_json::Value, crate::Error> {
        Ok(serde_json::to_value(value)?)
    }

    #[test]
    fn json_to_cbor_roundtrip() {
        let json = serde_json::json!({"foo": 8, "bar": [1, 2], "baz": "ciao"});
        let cbor = json_to_cbor(&json).unwrap();
        let back = cbor_to_json(cbor).unwrap();
        assert_eq!(json, back);
    }
}
