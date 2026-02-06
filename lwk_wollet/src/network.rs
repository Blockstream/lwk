// TODO: this should be removed in favor of the `lwk_common`. Currently consts from here are
// duplicated in the `lwk_common`.

use elements::hashes::Hash;
use serde::{Deserialize, Serialize};

use crate::elements::{AddressParams, AssetId, BlockHash};
use std::str::FromStr;

pub const LIQUID_POLICY_ASSET_STR: &str =
    "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";
pub const LIQUID_TESTNET_POLICY_ASSET_STR: &str =
    "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";
pub const LIQUID_DEFAULT_REGTEST_ASSET_STR: &str =
    "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";

// Genesis blockhashes for Liquid chains.
// Note these are binary format, not display format which is reversed
// taken and adapted from jade network.c
pub const GENESIS_LIQUID: [u8; 32] = [
    0x03, 0x60, 0x20, 0x8a, 0x88, 0x96, 0x92, 0x37, 0x2c, 0x8d, 0x68, 0xb0, 0x84, 0xa6, 0x2e, 0xfd,
    0xf6, 0x0e, 0xa1, 0xa3, 0x59, 0xa0, 0x4c, 0x94, 0xb2, 0x0d, 0x22, 0x36, 0x58, 0x27, 0x66, 0x14,
];
pub const GENESIS_LIQUID_TESTNET: [u8; 32] = [
    0xc1, 0xb1, 0x6a, 0xe2, 0x4f, 0x24, 0x23, 0xae, 0xa2, 0xea, 0x34, 0x55, 0x22, 0x92, 0x79, 0x3b,
    0x5b, 0x5e, 0x82, 0x99, 0x9a, 0x1e, 0xed, 0x81, 0xd5, 0x6a, 0xee, 0x52, 0x8e, 0xda, 0x71, 0xa7,
];
pub const GENESIS_LIQUID_REGTEST: [u8; 32] = [
    0x21, 0xca, 0xb1, 0xe5, 0xda, 0x47, 0x18, 0xea, 0x14, 0x0d, 0x97, 0x16, 0x93, 0x17, 0x02, 0x42,
    0x2f, 0x0e, 0x6a, 0xd9, 0x15, 0xc8, 0xd9, 0xb5, 0x83, 0xca, 0xc2, 0x70, 0x6b, 0x2a, 0x90, 0x00,
];

/// The network of the elements blockchain.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Copy, Hash)]
pub enum ElementsNetwork {
    /// Liquid mainnet.
    Liquid,
    /// Liquid testnet.
    LiquidTestnet,
    /// Liquid regtest with a custom policy asset.
    ElementsRegtest {
        /// The policy asset to use for this regtest network.
        /// You can use the default one using [`ElementsNetwork::default_regtest()`].
        policy_asset: AssetId,
    },
}

impl ElementsNetwork {
    /// Return the policy asset for this network.
    pub fn policy_asset(&self) -> AssetId {
        match self {
            ElementsNetwork::Liquid => {
                AssetId::from_str(LIQUID_POLICY_ASSET_STR).expect("can't fail on const")
            }
            ElementsNetwork::LiquidTestnet => {
                AssetId::from_str(LIQUID_TESTNET_POLICY_ASSET_STR).expect("can't fail on const")
            }
            ElementsNetwork::ElementsRegtest { policy_asset } => *policy_asset,
        }
    }

    /// Return the genesis block hash for this network.
    pub fn genesis_block_hash(&self) -> BlockHash {
        match self {
            ElementsNetwork::Liquid => BlockHash::from_byte_array(GENESIS_LIQUID),
            ElementsNetwork::LiquidTestnet => BlockHash::from_byte_array(GENESIS_LIQUID_TESTNET),
            ElementsNetwork::ElementsRegtest { .. } => {
                BlockHash::from_byte_array(GENESIS_LIQUID_REGTEST)
            }
        }
    }

    /// Return the string representation of this network.
    pub fn as_str(&self) -> &'static str {
        match self {
            ElementsNetwork::Liquid => "liquid",
            ElementsNetwork::LiquidTestnet => "liquid-testnet",
            ElementsNetwork::ElementsRegtest { .. } => "liquid-regtest",
        }
    }

    /// Return the address parameters for this network to generate addresses compatible for this network.
    pub fn address_params(&self) -> &'static AddressParams {
        match self {
            ElementsNetwork::Liquid => &AddressParams::LIQUID,
            ElementsNetwork::LiquidTestnet => &AddressParams::LIQUID_TESTNET,
            ElementsNetwork::ElementsRegtest { .. } => &AddressParams::ELEMENTS,
        }
    }

    /// Return the default regtest network using the default regtest policy asset.
    pub fn default_regtest() -> ElementsNetwork {
        let policy_asset = AssetId::from_str(LIQUID_DEFAULT_REGTEST_ASSET_STR).expect("static");

        ElementsNetwork::ElementsRegtest { policy_asset }
    }

    /// Return the dynamic epoch length of this network
    pub fn dynamic_epoch_length(&self) -> u32 {
        // taken from elements chainparams.cpp
        // TODO upstream to rust elements
        match self {
            ElementsNetwork::Liquid => 20160,
            ElementsNetwork::LiquidTestnet => 1000,
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => 10,
        }
    }

    /// Return the dynamic epoch length of this network
    pub fn total_valid_epochs(&self) -> u32 {
        // taken from elements chainparams.cpp
        // TODO upstream to rust elements
        match self {
            ElementsNetwork::Liquid => 2,
            ElementsNetwork::LiquidTestnet => 0,
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => 0,
        }
    }

    #[cfg(feature = "bindings")]
    /// Return the transaction builder for this network.
    pub fn tx_builder(&self) -> crate::TxBuilder {
        crate::TxBuilder::new(*self)
    }
}

impl From<ElementsNetwork> for lwk_common::Network {
    fn from(network: ElementsNetwork) -> Self {
        match network {
            ElementsNetwork::Liquid => lwk_common::Network::Liquid,
            ElementsNetwork::LiquidTestnet => lwk_common::Network::TestnetLiquid,
            ElementsNetwork::ElementsRegtest { policy_asset: _ } => {
                lwk_common::Network::LocaltestLiquid
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
        str::FromStr,
    };

    use super::ElementsNetwork;

    #[test]
    fn test_config_hash() {
        // Old Config struct had a single field,
        // so its hash is the same as the field hash
        #[derive(Hash)]
        struct Config {
            network: ElementsNetwork,
        }
        let network = ElementsNetwork::Liquid;
        let config = Config { network };
        let mut hasher = DefaultHasher::new();
        config.hash(&mut hasher);
        assert_eq!(13646096770106105413, hasher.finish());

        let mut hasher = DefaultHasher::new();
        network.hash(&mut hasher);
        assert_eq!(13646096770106105413, hasher.finish());
    }

    #[test]
    fn test_genesis_block_hash() {
        let network = ElementsNetwork::Liquid;
        assert_eq!(
            network.genesis_block_hash(),
            elements::BlockHash::from_str(
                "1466275836220db2944ca059a3a10ef6fd2ea684b0688d2c379296888a206003"
            )
            .unwrap()
        );

        let network = ElementsNetwork::LiquidTestnet;
        assert_eq!(
            network.genesis_block_hash(),
            elements::BlockHash::from_str(
                "a771da8e52ee6ad581ed1e9a99825e5b3b7992225534eaa2ae23244fe26ab1c1"
            )
            .unwrap()
        );

        let network = ElementsNetwork::default_regtest();
        assert_eq!(
            network.genesis_block_hash(),
            elements::BlockHash::from_str(
                "00902a6b70c2ca83b5d9c815d96a0e2f4202179316970d14ea1847dae5b1ca21"
            )
            .unwrap()
        );
    }
}
