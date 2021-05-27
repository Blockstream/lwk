use crate::error::Error;

use elements::Script;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use elements::bitcoin::hashes::hex::FromHex;
use elements::OutPoint;
use std::fmt::{Debug, Display};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Unblinded {
    pub asset: elements::issuance::AssetId,
    #[serde(with = "hex_rev")]
    pub asset_blinder: secp256k1_zkp::Tweak,
    #[serde(with = "hex_rev")]
    #[serde(rename = "amount_blinder")]
    pub value_blinder: secp256k1_zkp::Tweak,
    #[serde(rename = "amount")]
    pub value: u64,
}

mod hex_rev {
    use std::fmt::Write;

    // from secp256k1_zkp src/lib.rs
    fn from_hex(hex: &str, target: &mut [u8]) -> Result<usize, ()> {
        if hex.len() % 2 == 1 || hex.len() > target.len() * 2 {
            return Err(());
        }

        let mut b = 0;
        let mut idx = 0;
        for c in hex.bytes() {
            b <<= 4;
            match c {
                b'A'..=b'F' => b |= c - b'A' + 10,
                b'a'..=b'f' => b |= c - b'a' + 10,
                b'0'..=b'9' => b |= c - b'0',
                _ => return Err(()),
            }
            if (idx & 1) == 1 {
                target[idx / 2] = b;
                b = 0;
            }
            idx += 1;
        }
        Ok(idx / 2)
    }

    fn hex2tweak_rev<E>(v: &str) -> Result<secp256k1_zkp::Tweak, E>
    where
        E: ::serde::de::Error,
    {
        let mut res = [0u8; 32];
        match from_hex(v, &mut res) {
            Ok(32) => {
                res.reverse();
                secp256k1_zkp::Tweak::from_inner(res).map_err(E::custom)
            }
            _ => Err(E::custom("invalid hex")),
        }
    }

    // Adapted from rust-secp256k1-zkp src/zkp/generator.rs
    pub fn serialize<S: ::serde::Serializer>(
        blinder: &secp256k1_zkp::Tweak,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            let mut h = String::new();
            for e in blinder.as_ref().iter().rev() {
                write!(&mut h, "{:02x}", e).unwrap();
            }
            s.collect_str(&h)
        } else {
            s.serialize_bytes(blinder.as_ref())
        }
    }

    pub fn deserialize<'de, D: ::serde::Deserializer<'de>>(
        d: D,
    ) -> Result<secp256k1_zkp::Tweak, D::Error> {
        if d.is_human_readable() {
            struct HexVisitor;

            impl<'de> ::serde::de::Visitor<'de> for HexVisitor {
                type Value = secp256k1_zkp::Tweak;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("an ASCII hex string")
                }

                fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                where
                    E: ::serde::de::Error,
                {
                    if let Ok(hex) = std::str::from_utf8(v) {
                        hex2tweak_rev::<E>(hex)
                    } else {
                        Err(E::invalid_value(::serde::de::Unexpected::Bytes(v), &self))
                    }
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: ::serde::de::Error,
                {
                    hex2tweak_rev::<E>(v)
                }
            }
            d.deserialize_str(HexVisitor)
        } else {
            struct BytesVisitor;

            impl<'de> ::serde::de::Visitor<'de> for BytesVisitor {
                type Value = secp256k1_zkp::Tweak;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a bytestring")
                }

                fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
                where
                    E: ::serde::de::Error,
                {
                    secp256k1_zkp::Tweak::from_slice(v).map_err(E::custom)
                }
            }

            d.deserialize_bytes(BytesVisitor)
        }
    }
}

impl Unblinded {
    pub fn commitments(
        &self,
        secp: &secp256k1_zkp::Secp256k1<secp256k1_zkp::All>,
    ) -> (secp256k1_zkp::Generator, secp256k1_zkp::PedersenCommitment) {
        let asset_tag = secp256k1_zkp::Tag::from(self.asset.into_inner().into_inner());
        let asset_generator =
            secp256k1_zkp::Generator::new_blinded(secp, asset_tag, self.asset_blinder);
        let value_commitment = secp256k1_zkp::PedersenCommitment::new(
            secp,
            self.value,
            self.value_blinder,
            asset_generator,
        );
        (asset_generator, value_commitment)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TXO {
    pub outpoint: OutPoint,
    pub script_pubkey: Script,
    pub height: Option<u32>,
}

impl TXO {
    pub fn new(outpoint: OutPoint, script_pubkey: Script, height: Option<u32>) -> TXO {
        TXO {
            outpoint,
            script_pubkey,
            height,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnblindedTXO {
    pub txo: TXO,
    pub unblinded: Unblinded,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransactionDetails {
    pub transaction: elements::Transaction,
    pub txid: String,
    pub balances: HashMap<elements::issuance::AssetId, i64>,
    pub fee: u64,
    pub height: Option<u32>,
    pub spv_verified: SPVVerifyResult,
}

impl TransactionDetails {
    pub fn new(
        transaction: elements::Transaction,
        balances: HashMap<elements::issuance::AssetId, i64>,
        fee: u64,
        height: Option<u32>,
        spv_verified: SPVVerifyResult,
    ) -> TransactionDetails {
        let txid = transaction.txid().to_string();
        TransactionDetails {
            transaction,
            txid,
            balances,
            fee,
            height,
            spv_verified,
        }
    }

    pub fn hex(&self) -> String {
        hex::encode(elements::encode::serialize(&self.transaction))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Destination {
    address: elements::Address,
    satoshi: u64,
    asset: elements::issuance::AssetId,
}

impl Destination {
    pub fn new(address: &str, satoshi: u64, asset: &str) -> Result<Self, Error> {
        let address = elements::Address::from_str(address).map_err(|_| Error::InvalidAddress)?;
        let asset = elements::issuance::AssetId::from_hex(asset)?;
        Ok(Destination {
            address,
            satoshi,
            asset,
        })
    }

    pub fn address(&self) -> elements::Address {
        self.address.clone()
    }

    pub fn satoshi(&self) -> u64 {
        self.satoshi
    }

    pub fn asset(&self) -> elements::issuance::AssetId {
        self.asset
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CreateTransactionOpt {
    // TODO: chage type to hold SendAll and be valid
    pub addressees: Vec<Destination>,
    pub fee_rate: Option<u64>, // in satoshi/kbyte
    pub utxos: Option<Vec<UnblindedTXO>>,
}
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct GetTransactionsOpt {
    pub first: usize,
    pub count: usize,
    pub subaccount: usize,
    pub num_confs: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SPVVerifyResult {
    InProgress,
    Verified,
    NotVerified,
    Disabled,
}

// This one is simple enough to derive a serializer
#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct FeeEstimate(pub u64);

impl SPVVerifyResult {
    pub fn as_i32(&self) -> i32 {
        match self {
            SPVVerifyResult::InProgress => 0,
            SPVVerifyResult::Verified => 1,
            SPVVerifyResult::NotVerified => 2,
            SPVVerifyResult::Disabled => 3,
        }
    }
}

impl Display for SPVVerifyResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SPVVerifyResult::InProgress => write!(f, "in_progress"),
            SPVVerifyResult::Verified => write!(f, "verified"),
            SPVVerifyResult::NotVerified => write!(f, "not_verified"),
            SPVVerifyResult::Disabled => write!(f, "disabled"),
        }
    }
}

#[cfg(test)]
mod tests {
    use elements::bitcoin::hashes::hex::{FromHex, ToHex};

    #[test]
    fn test_asset_roundtrip() {
        let hex = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
        let asset = elements::issuance::AssetId::from_hex(&hex).unwrap();
        assert_eq!(asset.to_hex(), hex);
    }
}
