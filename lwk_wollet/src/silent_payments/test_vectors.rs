//! The BIP352 test vectors.
//!
//! They cover the key derivation, which is the same on Liquid, so they are used to check the
//! whole protocol but the blinding.

use std::str::FromStr;

use bech32::Hrp;
use elements::encode::deserialize;
use elements::hex::FromHex;
use elements::{OutPoint, Script, Sequence, Transaction, TxIn, TxInWitness, TxOut, Txid};
use serde::Deserialize;

use crate::secp256k1::{PublicKey, SecretKey};
use crate::EC;

use super::{SilentPaymentAddress, SilentPaymentInput, SilentPaymentNetwork, SilentPaymentScanner};

#[derive(Deserialize)]
pub struct TestVector {
    pub comment: String,
    pub sending: Vec<Sending>,
    pub receiving: Vec<Receiving>,
}

#[derive(Deserialize)]
pub struct Sending {
    pub given: SendingGiven,
    pub expected: SendingExpected,
}

#[derive(Deserialize)]
pub struct SendingGiven {
    pub vin: Vec<Vin>,
    pub recipients: Vec<Recipient>,
}

#[derive(Deserialize)]
pub struct SendingExpected {
    /// Every element is a valid set of outputs, the order within a set is not specified
    pub outputs: Vec<Vec<String>>,
}

#[derive(Deserialize)]
pub struct Recipient {
    pub address: String,

    /// How many outputs pay to this address
    #[serde(default = "one")]
    pub count: usize,
}

fn one() -> usize {
    1
}

#[derive(Deserialize)]
pub struct Receiving {
    pub given: ReceivingGiven,
    pub expected: ReceivingExpected,
}

#[derive(Deserialize)]
pub struct ReceivingGiven {
    pub vin: Vec<Vin>,
    pub outputs: Vec<String>,
    pub key_material: KeyMaterial,
    pub labels: Vec<u32>,
}

#[derive(Deserialize)]
pub struct KeyMaterial {
    pub spend_priv_key: String,
    pub scan_priv_key: String,
}

#[derive(Deserialize)]
pub struct ReceivingExpected {
    pub addresses: Vec<String>,

    #[serde(default)]
    pub outputs: Vec<ExpectedOutput>,

    /// Used instead of `outputs` when they are too many to be listed
    #[serde(default)]
    pub n_outputs: Option<usize>,

    pub tweak: Option<String>,

    #[serde(default)]
    pub input_pub_key_sum: Option<String>,
}

#[derive(Deserialize)]
pub struct ExpectedOutput {
    pub priv_key_tweak: String,
    pub pub_key: String,
}

#[derive(Deserialize)]
pub struct Vin {
    pub txid: String,
    pub vout: u32,

    #[serde(rename = "scriptSig")]
    pub script_sig: String,

    pub txinwitness: String,

    pub prevout: Prevout,

    #[serde(default)]
    pub private_key: Option<String>,
}

#[derive(Deserialize)]
pub struct Prevout {
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: ScriptPubKey,
}

#[derive(Deserialize)]
pub struct ScriptPubKey {
    pub hex: String,
}

impl Vin {
    pub fn input(&self) -> SilentPaymentInput {
        let outpoint = OutPoint::new(Txid::from_str(&self.txid).unwrap(), self.vout);
        SilentPaymentInput::new(
            outpoint,
            script(&self.prevout.script_pub_key.hex),
            script(&self.script_sig),
            witness(&self.txinwitness),
        )
    }

    pub fn secret_key(&self) -> Option<SecretKey> {
        self.private_key.as_ref().map(|k| k.parse().unwrap())
    }
}

impl Recipient {
    pub fn address(&self) -> SilentPaymentAddress {
        parse_bip352_address(&self.address)
    }
}

impl Receiving {
    pub fn scanner(&self) -> SilentPaymentScanner {
        let scan = self.given.key_material.scan_priv_key.parse().unwrap();
        let spend: SecretKey = self.given.key_material.spend_priv_key.parse().unwrap();
        let mut scanner = SilentPaymentScanner::new(scan, PublicKey::from_secret_key(&EC, &spend));
        for label in &self.given.labels {
            scanner.add_label(*label).unwrap();
        }
        scanner
    }

    pub fn expected_outputs(&self) -> usize {
        self.expected
            .n_outputs
            .unwrap_or(self.expected.outputs.len())
    }
}

/// The BIP352 test vectors use the bitcoin human readable part, the payload is the same
pub fn parse_bip352_address(address: &str) -> SilentPaymentAddress {
    let (_, _, scan, spend) = SilentPaymentAddress::decode_with_hrp(address).unwrap();
    SilentPaymentAddress::new(scan, spend, SilentPaymentNetwork::Liquid)
}

/// See [`parse_bip352_address`]
pub fn encode_bip352_address(address: &SilentPaymentAddress) -> String {
    address.encode_with_hrp(Hrp::parse_unchecked("sp"))
}

pub fn test_vectors() -> Vec<TestVector> {
    serde_json::from_str(lwk_test_util::bip352_test_vectors()).unwrap()
}

pub fn script(hex: &str) -> Script {
    Script::from(Vec::<u8>::from_hex(hex).unwrap())
}

/// The taproot script paying to the given x-only public key
pub fn taproot_script(x_only: &str) -> Script {
    script(&format!("5120{x_only}"))
}

fn witness(hex: &str) -> Vec<Vec<u8>> {
    if hex.is_empty() {
        return vec![];
    }
    deserialize(&Vec::<u8>::from_hex(hex).unwrap()).unwrap()
}

/// A transaction spending the given inputs and paying to the given scripts.
///
/// Values are explicit: the BIP352 vectors are about key derivation, blinding is checked
/// against transactions built by the wallet itself.
pub fn transaction(inputs: &[SilentPaymentInput], scripts: &[Script]) -> Transaction {
    Transaction {
        version: 2,
        lock_time: elements::LockTime::ZERO,
        input: inputs
            .iter()
            .map(|input| TxIn {
                previous_output: input.outpoint(),
                is_pegin: false,
                script_sig: Script::new(),
                sequence: Sequence::MAX,
                asset_issuance: Default::default(),
                witness: TxInWitness::empty(),
            })
            .collect(),
        output: scripts
            .iter()
            .map(|script_pubkey| TxOut {
                script_pubkey: script_pubkey.clone(),
                ..TxOut::default()
            })
            .collect(),
    }
}
