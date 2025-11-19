use std::fmt::{self, Display};

use elements::bitcoin;

use crate::Network;

/// A wrapper around `elements::Address` that checks the network and provides a more user-friendly parse errors
#[derive(Debug)]
pub struct Address {
    inner: elements::Address,
}

impl From<elements::Address> for Address {
    fn from(inner: elements::Address) -> Self {
        Self { inner }
    }
}

impl From<Address> for elements::Address {
    fn from(addr: Address) -> Self {
        addr.inner
    }
}

/// The error type returned by [`Address::parse`]
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum AddressParseError {
    #[error("Expected a mainnet address but got a testnet one")]
    ExpectedMainnetGotTestnet,

    #[error("Expected a testnet address but got a mainnet one")]
    ExpectedTestnetGotMainnet,

    #[error("Expected elements address but got a bitcoin address")]
    ExpectedElementsGotBitcoin,

    #[error("Expected a blinded address but got a non-blinded one")]
    ExpectedBlindedAddress,

    #[error("Empty address string")]
    EmptyAddressString,

    #[error(transparent)]
    Elements(#[from] elements::AddressError),
}

impl AsRef<elements::Address> for Address {
    fn as_ref(&self) -> &elements::Address {
        &self.inner
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl Address {
    /// Parses an `Address` ensuring is for the right network
    pub fn parse(s: &str, network: Network) -> Result<Address, AddressParseError> {
        if s.is_empty() {
            return Err(AddressParseError::EmptyAddressString);
        }

        match s.parse::<elements::Address>() {
            Ok(inner) => {
                if inner.is_liquid() == network.is_mainnet() {
                    if inner.is_blinded() {
                        Ok(Address { inner })
                    } else {
                        Err(AddressParseError::ExpectedBlindedAddress)
                    }
                } else if network.is_mainnet() {
                    Err(AddressParseError::ExpectedMainnetGotTestnet)
                } else {
                    Err(AddressParseError::ExpectedTestnetGotMainnet)
                }
            }
            Err(elements_error) => {
                // Try parsing as bitcoin address
                match s.parse::<bitcoin::Address<bitcoin::address::NetworkUnchecked>>() {
                    Ok(_) => Err(AddressParseError::ExpectedElementsGotBitcoin),
                    Err(_) => Err(AddressParseError::Elements(elements_error)),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_liquid_mainnet_confidential_success() {
        let addr = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let result = Address::parse(addr, Network::Liquid);
        assert!(result.is_ok());
        let address = result.unwrap();
        assert_eq!(address.to_string(), addr);
    }

    #[test]
    fn test_parse_liquid_mainnet_unconfidential_fail() {
        let addr = "QLFdUboUPJnUzvsXKu83hUtrQ1DuxyggRg";
        let result = Address::parse(addr, Network::Liquid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AddressParseError::ExpectedBlindedAddress
        ));
    }

    #[test]
    fn test_parse_liquid_testnet_confidential_success() {
        let addr = "tlq1qqvzrwmpx0vq2kqcpnxtgszzlqqvyyqgfdqwjvd4s40rutvcf3zh0kuk6d549cauk00d7cv8tychpk9su4c0h7e0ukgq6ssd9q";
        let result = Address::parse(addr, Network::TestnetLiquid);
        assert!(result.is_ok());
        let address = result.unwrap();
        assert_eq!(address.to_string(), addr);
    }

    #[test]
    fn test_parse_liquid_testnet_unconfidential_fail() {
        let addr = "tex1qx90cvlsrluvqjj9uv54an54sv0kqfse45nyrxx";
        let result = Address::parse(addr, Network::TestnetLiquid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AddressParseError::ExpectedBlindedAddress
        ));
    }

    #[test]
    fn test_parse_mainnet_address_with_testnet_network() {
        let addr = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let result = Address::parse(addr, Network::TestnetLiquid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AddressParseError::ExpectedTestnetGotMainnet
        ));
    }

    #[test]
    fn test_parse_testnet_address_with_mainnet_network() {
        let addr = "tlq1qqvzrwmpx0vq2kqcpnxtgszzlqqvyyqgfdqwjvd4s40rutvcf3zh0kuk6d549cauk00d7cv8tychpk9su4c0h7e0ukgq6ssd9q";
        let result = Address::parse(addr, Network::Liquid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AddressParseError::ExpectedMainnetGotTestnet
        ));
    }

    #[test]
    fn test_parse_bitcoin_address_fail() {
        let addr = "bc1qwzrryqr3ja8w7hnja2spmkgfdcgvqwp5swz4af4ngsjecfz0w0pqud7k38";
        let result = Address::parse(addr, Network::Liquid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AddressParseError::ExpectedElementsGotBitcoin
        ));
    }

    #[test]
    fn test_parse_invalid_address_string() {
        let addr = "invalid_address_string";
        let result = Address::parse(addr, Network::Liquid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AddressParseError::Elements(_)
        ));
    }

    #[test]
    fn test_parse_empty_string() {
        let addr = "";
        let result = Address::parse(addr, Network::Liquid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AddressParseError::EmptyAddressString
        ));
    }

    #[test]
    fn test_address_display() {
        let addr = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let address = Address::parse(addr, Network::Liquid).unwrap();
        assert_eq!(format!("{address}"), addr);
    }

    #[test]
    fn test_address_as_ref() {
        let addr = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let address = Address::parse(addr, Network::Liquid).unwrap();
        let inner: &elements::Address = address.as_ref();
        assert_eq!(inner.to_string(), addr);
    }

    #[test]
    fn test_parse_localtest_liquid_network() {
        // Test with LocaltestLiquid network (regtest)
        let addr = "lq1qqduq2l8maf4580wle4hevmk62xqqw3quckshkt2rex3ylw83824y4g96xl0uugdz4qks5v7w4pdpvztyy5kw7r7e56jcwm0p0";
        let result = Address::parse(addr, Network::LocaltestLiquid);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AddressParseError::ExpectedTestnetGotMainnet
        ));
    }
}
