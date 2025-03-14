use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Liquid,
    TestnetLiquid,
    LocaltestLiquid,
}

impl Network {
    pub fn is_mainnet(&self) -> bool {
        self == &Self::Liquid
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
