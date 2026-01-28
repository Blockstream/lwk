use simplicityhl::elements::AddressParams;
use simplicityhl::simplicity::hashes::Hash;

/// Get the address parameters for a network.
pub fn network_to_address_params(network: lwk_common::Network) -> &'static AddressParams {
    match network {
        lwk_common::Network::Liquid => &AddressParams::LIQUID,
        lwk_common::Network::TestnetLiquid => &AddressParams::LIQUID_TESTNET,
        lwk_common::Network::LocaltestLiquid => &AddressParams::ELEMENTS,
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
