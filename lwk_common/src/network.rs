use std::str::FromStr;

use elements::hashes::{sha256, Hash};
use elements::{AddressParams, AssetId, BlockHash};
use serde::{Deserialize, Deserializer, Serialize};

const LIQUID_TESTNET_POLICY_ASSET: &AssetId = &AssetId::from_inner(sha256::Midstate([
    0x49, 0x9a, 0x81, 0x85, 0x45, 0xf6, 0xba, 0xe3, 0x9f, 0xc0, 0x3b, 0x63, 0x7f, 0x2a, 0x4e, 0x1e,
    0x64, 0xe5, 0x90, 0xca, 0xc1, 0xbc, 0x3a, 0x6f, 0x6d, 0x71, 0xaa, 0x44, 0x43, 0x65, 0x4c, 0x14,
]));

const LIQUID_REGTEST_POLICY_ASSET: &AssetId = &AssetId::from_inner(sha256::Midstate([
    0x25, 0xb2, 0x51, 0x07, 0x0e, 0x29, 0xca, 0x19, 0x04, 0x3c, 0xf3, 0x3c, 0xcd, 0x73, 0x24, 0xe2,
    0xdd, 0xab, 0x03, 0xec, 0xc4, 0xae, 0x0b, 0x5e, 0x77, 0xc4, 0xfc, 0x0e, 0x5c, 0xf6, 0xc9, 0x5a,
]));

// Note these are binary format, not display format which is reversed.
// Taken and adapted from jade network.c.
const GENESIS_LIQUID: [u8; 32] = [
    0x03, 0x60, 0x20, 0x8a, 0x88, 0x96, 0x92, 0x37, 0x2c, 0x8d, 0x68, 0xb0, 0x84, 0xa6, 0x2e, 0xfd,
    0xf6, 0x0e, 0xa1, 0xa3, 0x59, 0xa0, 0x4c, 0x94, 0xb2, 0x0d, 0x22, 0x36, 0x58, 0x27, 0x66, 0x14,
];
const GENESIS_LIQUID_TESTNET: [u8; 32] = [
    0xc1, 0xb1, 0x6a, 0xe2, 0x4f, 0x24, 0x23, 0xae, 0xa2, 0xea, 0x34, 0x55, 0x22, 0x92, 0x79, 0x3b,
    0x5b, 0x5e, 0x82, 0x99, 0x9a, 0x1e, 0xed, 0x81, 0xd5, 0x6a, 0xee, 0x52, 0x8e, 0xda, 0x71, 0xa7,
];
// Regtest genesis hash is based on the `lwk_test_util/src/test_env.rs` setup.
const GENESIS_LIQUID_REGTEST: [u8; 32] = [
    0xf7, 0x6a, 0xfd, 0x8b, 0xc7, 0xd9, 0xc5, 0x45, 0x33, 0x16, 0xca, 0x69, 0x0b, 0xd4, 0x3f, 0x63,
    0xc1, 0x45, 0x10, 0xd4, 0x2b, 0x90, 0x74, 0xa5, 0x98, 0x34, 0x4a, 0x77, 0xb0, 0x03, 0xaf, 0xc7,
];

/// The network of the elements blockchain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// Liquid mainnet
    Liquid,
    /// Liquid testnet
    TestnetLiquid,
    /// Liquid regtest
    LocaltestLiquid,
}

impl Network {
    /// Return true if the network is mainnet.
    pub fn is_mainnet(&self) -> bool {
        self == &Self::Liquid
    }

    /// Return the policy asset for this network.
    pub fn policy_asset(&self) -> &'static AssetId {
        match self {
            Network::Liquid => &AssetId::LIQUID_BTC,
            Network::TestnetLiquid => LIQUID_TESTNET_POLICY_ASSET,
            Network::LocaltestLiquid => LIQUID_REGTEST_POLICY_ASSET,
        }
    }

    /// Return the genesis block hash for this network.
    pub fn genesis_hash(&self) -> BlockHash {
        match self {
            Network::Liquid => BlockHash::from_byte_array(GENESIS_LIQUID),
            Network::TestnetLiquid => BlockHash::from_byte_array(GENESIS_LIQUID_TESTNET),
            Network::LocaltestLiquid => BlockHash::from_byte_array(GENESIS_LIQUID_REGTEST),
        }
    }

    /// Return the address parameters for this network to generate addresses compatible for this network.
    pub fn address_params(&self) -> &'static AddressParams {
        match self {
            Network::Liquid => &AddressParams::LIQUID,
            Network::TestnetLiquid => &AddressParams::LIQUID_TESTNET,
            Network::LocaltestLiquid => &AddressParams::ELEMENTS,
        }
    }
}

impl std::fmt::Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Liquid => write!(f, "liquid"),
            Network::TestnetLiquid => write!(f, "testnet-liquid"),
            Network::LocaltestLiquid => write!(f, "localtest-liquid"),
        }
    }
}

impl FromStr for Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "liquid" => Ok(Network::Liquid),
            "testnet-liquid" => Ok(Network::TestnetLiquid),
            "localtest-liquid" => Ok(Network::LocaltestLiquid),
            _ => Err(
                "invalid network, possible value are: 'liquid', 'testnet-liquid', 'localtest-liquid'"
                    .to_string(),
            ),
        }
    }
}

impl Serialize for Network {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Network {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let string = String::deserialize(d)?;
        string.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_constants_regression() {
        // Policy asset display matches expected hex
        assert_eq!(
            Network::Liquid.policy_asset().to_string(),
            "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d"
        );
        assert_eq!(
            Network::TestnetLiquid.policy_asset().to_string(),
            "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49"
        );
        assert_eq!(
            Network::LocaltestLiquid.policy_asset().to_string(),
            "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225"
        );

        // Genesis block hash display (reversed hex) as seen on explorers/nodes
        // Took from: https://github.com/ElementsProject/elements/blob/6bb916a57fa8b677bd8060491ea7ab28b77794ff/src/chainparams.cpp#L1323
        assert_eq!(
            Network::Liquid.genesis_hash().to_string(),
            "1466275836220db2944ca059a3a10ef6fd2ea684b0688d2c379296888a206003"
        );
        assert_eq!(
            Network::TestnetLiquid.genesis_hash().to_string(),
            "a771da8e52ee6ad581ed1e9a99825e5b3b7992225534eaa2ae23244fe26ab1c1"
        );
        assert_eq!(
            Network::LocaltestLiquid.genesis_hash().to_string(),
            "c7af03b0774a3498a574902bd41045c1633fd40b69ca163345c5d9c78bfd6af7"
        );
    }
}
