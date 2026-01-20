use std::collections::HashMap;

use simplicityhl::elements::AddressParams;
use simplicityhl::num::U256;
use simplicityhl::simplicity::hashes::Hash;
use simplicityhl::str::WitnessName;
use simplicityhl::value::ValueConstructible;
use simplicityhl::Value;

/// A value that can be passed as a Simplicity argument or witness.
#[derive(Clone)]
pub enum SimplicityValue {
    /// Numeric value (handles u8, u16, u32, u64 - simplicityhl coerces as needed)
    Number(u64),
    /// Byte array (32 bytes = u256, 64 bytes = signature)
    Bytes(Vec<u8>),
}

/// Validate byte length for Simplicity values.
/// Returns error message if invalid, None if valid.
pub fn validate_bytes_length(len: usize) -> Option<String> {
    if len != 32 && len != 64 {
        Some(format!(
            "bytes must be exactly 32 (u256) or 64 (signature) bytes, got {}",
            len
        ))
    } else {
        None
    }
}

/// Convert a HashMap of SimplicityValue to the simplicityhl Value format.
pub fn convert_values_to_map(
    values: &HashMap<String, SimplicityValue>,
) -> HashMap<WitnessName, Value> {
    values
        .iter()
        .map(|(name, value)| {
            let witness_name = WitnessName::from_str_unchecked(name);
            let val = match value {
                SimplicityValue::Number(v) => Value::u64(*v),
                SimplicityValue::Bytes(bytes) => {
                    if bytes.len() == 32 {
                        let arr: [u8; 32] = bytes.as_slice().try_into().unwrap();
                        Value::u256(U256::from_byte_array(arr))
                    } else if bytes.len() == 64 {
                        let arr: [u8; 64] = bytes.as_slice().try_into().unwrap();
                        Value::byte_array(arr)
                    } else {
                        unreachable!("byte length validated in add_bytes")
                    }
                }
            };
            (witness_name, val)
        })
        .collect()
}

/// Network type for address parameter selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkKind {
    Liquid,
    LiquidTestnet,
    ElementsRegtest,
}

/// Get the address parameters for a network.
pub fn network_to_address_params(network: NetworkKind) -> &'static AddressParams {
    match network {
        NetworkKind::Liquid => &AddressParams::LIQUID,
        NetworkKind::LiquidTestnet => &AddressParams::LIQUID_TESTNET,
        NetworkKind::ElementsRegtest => &AddressParams::ELEMENTS,
    }
}

/// Parse a genesis hash from hex bytes (display format) to BlockHash.
/// The bytes are in big-endian (display) format, but BlockHash::from_byte_array
/// expects little-endian (internal) format, so we need to reverse the bytes.
pub fn parse_genesis_hash(
    genesis_bytes: &[u8],
) -> Result<simplicityhl::elements::BlockHash, &'static str> {
    let mut arr: [u8; 32] = genesis_bytes
        .try_into()
        .map_err(|_| "genesis_hash must be exactly 32 bytes")?;

    arr.reverse();

    Ok(simplicityhl::elements::BlockHash::from_byte_array(arr))
}
