#![doc = include_str!("../README.md")]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "asyncr")]
pub mod asyncr;

mod anti_exfil;
pub mod consts;
pub mod error;
pub mod get_receive_address;
pub mod protocol;
pub mod register_multisig;
pub mod sign_liquid_tx;
mod sign_message;
mod sign_pset_common;

#[cfg(feature = "test_emulator")]
mod jade_emulator;

#[cfg(feature = "test_emulator")]
pub use jade_emulator::{TestJadeEmulator, TestJadeEmulatorGuard};

#[cfg(feature = "sync")]
mod sync;

use std::{collections::HashSet, sync::LazyLock};

pub use consts::{BAUD_RATE, TIMEOUT};
use elements::{
    bitcoin::bip32::{ChildNumber, DerivationPath, Fingerprint},
    encode::serialize,
    hashes::Hash,
    hex::ToHex,
    opcodes::{
        all::{OP_CHECKMULTISIG, OP_PUSHNUM_1, OP_PUSHNUM_16},
        All,
    },
    pset::{serialize::Serialize, PartiallySignedTransaction},
    script::Instruction,
    secp256k1_zkp::{Secp256k1, VerifyOnly},
    Script,
};
pub use error::Error;
use error::ErrorDetails;
use get_receive_address::{SingleOrMulti, Variant};
use lwk_common::{get_genesis_hash, Network};

use register_multisig::RegisteredMultisigDetails;
use serde::Deserialize;
use sign_liquid_tx::{AssetInfo, Change, Commitment, Contract, Prevout, SignLiquidTxParams};

pub(crate) static SECP: LazyLock<Secp256k1<VerifyOnly>> =
    LazyLock::new(Secp256k1::verification_only);

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
pub const JADE_DEVICE_IDS: [(u16, u16); 6] = [
    (0x10c4, 0xea60),
    (0x1a86, 0x55d4),
    (0x0403, 0x6001),
    (0x1a86, 0x7523),
    // new
    (0x303a, 0x4001),
    (0x303a, 0x1001),
];

const CHANGE_CHAIN: ChildNumber = ChildNumber::Normal { index: 1 };

/// Id Jade replies with when it cannot recover the id of the request it is rejecting.
///
/// Set by `jade_process_reject_message_ex`
const UNMATCHED_REQUEST_ID: &str = "00";

/// Outcome of trying to decode a message out of the bytes read so far.
enum ParseStep<T> {
    Incomplete,
    Mine(Result<protocol::Response<T>>),
    Skip { consumed: usize },
}

/// Read the `id` of a message, if it has a usable one.
fn message_id(value: &serde_cbor::Value) -> Option<&str> {
    let serde_cbor::Value::Map(map) = value else {
        return None;
    };
    match map.get(&serde_cbor::Value::Text("id".to_string())) {
        Some(serde_cbor::Value::Text(id)) => Some(id),
        _ => None,
    }
}

/// Read the `error` of a message, if it has one.
fn message_error(value: &serde_cbor::Value) -> Option<&serde_cbor::Value> {
    let serde_cbor::Value::Map(map) = value else {
        return None;
    };
    map.get(&serde_cbor::Value::Text("error".to_string()))
}

/// Decode the first message in `reader`, given the id of the request waiting for an answer.
fn try_parse_response<T>(reader: &[u8], expected_id: &str) -> ParseStep<T>
where
    T: std::fmt::Debug + serde::de::DeserializeOwned,
{
    let mut deserializer = serde_cbor::Deserializer::from_slice(reader);
    let value = match serde_cbor::Value::deserialize(&mut deserializer) {
        Ok(value) => value,
        Err(e) => {
            return if e.is_eof() {
                ParseStep::Incomplete
            } else {
                ParseStep::Mine(Err(Error::SerdeCbor(e)))
            };
        }
    };
    let consumed = deserializer.byte_offset();

    log::debug!(
        "\n<---\t{:?}\n\t({} bytes) {}",
        &value,
        consumed,
        hex::encode(&reader[..consumed])
    );

    let Some(id) = message_id(&value) else {
        log::warn!("Skipping message without an id: {value:?}");
        return ParseStep::Skip { consumed };
    };

    if id == UNMATCHED_REQUEST_ID {
        let Some(details) = message_error(&value) else {
            log::warn!("Skipping message with id {UNMATCHED_REQUEST_ID} and no error: {value:?}");
            return ParseStep::Skip { consumed };
        };
        return ParseStep::Mine(Err(
            match serde_cbor::value::from_value::<ErrorDetails>(details.clone()) {
                Ok(details) => Error::JadeError(details),
                Err(e) => Error::SerdeCbor(e),
            },
        ));
    }

    if id != expected_id {
        log::warn!("Skipping stale response, expected id {expected_id}, got {id}");
        return ParseStep::Skip { consumed };
    }

    match serde_cbor::value::from_value::<protocol::Response<T>>(value) {
        Ok(response) => ParseStep::Mine(Ok(response)),
        Err(e) => {
            log::warn!("The value returned is a valid CBOR, but our structs doesn't map it correctly: {e:?}");
            ParseStep::Mine(Err(Error::SerdeCbor(e)))
        }
    }
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

fn create_jade_sign_req(
    pset: &mut PartiallySignedTransaction,
    my_fingerprint: Fingerprint,
    multisigs_details: Vec<RegisteredMultisigDetails>,
    network: Network,
) -> Result<SignLiquidTxParams> {
    let tx = pset.extract_tx()?;
    let txn = serialize(&tx);
    let mut asset_ids_in_tx = HashSet::new();
    let mut trusted_commitments = vec![];
    let mut changes = vec![];
    for (i, output) in pset.outputs().iter().enumerate() {
        let asset_id = output.asset.ok_or(Error::MissingAssetIdInOutput(i))?;
        asset_ids_in_tx.insert(asset_id);
        let mut asset_id = serialize(&asset_id);
        asset_id.reverse(); // Jade want it reversed
        let trusted_commitment = if !output.is_partially_blinded() {
            // This can be a fee output, burn output, or explicit normal output.
            // Jade requires trusted commitments for blinded outputs, but allows null
            // entries for unblinded outputs. When no commitment data is present and
            // the tx output asset/value are explicit, Jade reads the asset/value
            // directly from the transaction output.
            // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/docs/index.rst#sign_liquid_tx-request-legacy (see bullets section)
            None
        } else {
            Some(Commitment {
                asset_blind_proof: output
                    .blind_asset_proof
                    .as_ref()
                    .ok_or(Error::MissingBlindAssetProofInOutput(i))?
                    .serialize(),
                asset_generator: output
                    .asset_comm
                    .ok_or(Error::MissingAssetCommInOutput(i))?
                    .serialize()
                    .to_vec(),
                asset_id,
                blinding_key: output
                    .blinding_key
                    .ok_or(Error::MissingBlindingKeyInOutput(i))?
                    .to_bytes(),
                value: output.amount.ok_or(Error::MissingAmountInOutput(i))?,
                value_commitment: output
                    .amount_comm
                    .ok_or(Error::MissingAmountCommInOutput(i))?
                    .serialize()
                    .to_vec(),
                value_blind_proof: output
                    .blind_value_proof
                    .as_ref()
                    .ok_or(Error::MissingBlindValueProofInOutput(i))?
                    .serialize(),
            })
        };
        trusted_commitments.push(trusted_commitment);

        let mut change = None;
        for (_, (_, (fingerprint, path))) in output.tap_key_origins.iter() {
            if fingerprint == &my_fingerprint {
                let is_change = path.clone().into_iter().nth_back(1) == Some(&CHANGE_CHAIN);
                if is_change && output.script_pubkey.is_v1_p2tr() {
                    change = Some(Change {
                        address: SingleOrMulti::Single {
                            variant: Variant::Tr,
                            path: derivation_path_to_vec(path),
                        },
                        is_change,
                    });
                }
            }
        }
        for (fingerprint, path) in output.bip32_derivation.values() {
            if fingerprint == &my_fingerprint {
                // This ensures that path has at least 2 elements
                let is_change = path.clone().into_iter().nth_back(1) == Some(&CHANGE_CHAIN);
                if is_change {
                    if output.script_pubkey.is_v0_p2wpkh() {
                        change = Some(Change {
                            address: SingleOrMulti::Single {
                                variant: Variant::Wpkh,
                                path: derivation_path_to_vec(path),
                            },
                            is_change,
                        });
                    } else if output.script_pubkey.is_p2sh() {
                        if let Some(redeem_script) = output.redeem_script.as_ref() {
                            if redeem_script.is_v0_p2wpkh() {
                                change = Some(Change {
                                    address: SingleOrMulti::Single {
                                        variant: Variant::ShWpkh,
                                        path: derivation_path_to_vec(path),
                                    },
                                    is_change,
                                });
                            }
                        }
                    } else if output.script_pubkey.is_v0_p2wsh() {
                        if let Some(witness_script) = output.witness_script.as_ref() {
                            if is_multisig(witness_script) {
                                for details in &multisigs_details {
                                    // path has at least 2 elements
                                    let index = path[path.len() - 1];
                                    if let Ok(derived_witness_script) = details
                                        .descriptor
                                        .derive_witness_script(is_change, index.into())
                                    {
                                        if witness_script == &derived_witness_script {
                                            let mut paths = vec![];
                                            for _ in 0..details.descriptor.signers.len() {
                                                // FIXME: here we should only pass the paths that were
                                                // not passed when calling register_multisig. However
                                                // deducing them now is not trivial, thus we only take
                                                // the last 2 elements in the derivation path which we
                                                // expect to be "0|1,*"
                                                let v = derivation_path_to_vec(path);
                                                // path has at least 2 elements
                                                let v = v[(path.len() - 2)..].to_vec();
                                                paths.push(v);
                                            }
                                            change = Some(Change {
                                                address: SingleOrMulti::Multi {
                                                    multisig_name: details
                                                        .multisig_name
                                                        .to_string(),
                                                    paths,
                                                },
                                                is_change,
                                            });
                                            break; // No need to check for more multisigs
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        changes.push(change);
    }
    let mut assets_info = vec![];
    for asset_id in asset_ids_in_tx {
        if let Some(Ok(meta)) = pset.get_asset_metadata(asset_id) {
            if let Ok(contract) = serde_json::from_str::<Contract>(meta.contract()) {
                let asset_info = AssetInfo {
                    asset_id: asset_id.to_string(),
                    contract,
                    issuance_prevout: Prevout {
                        txid: meta.issuance_prevout().txid.to_hex(),
                        vout: meta.issuance_prevout().vout,
                    },
                };

                assets_info.push(asset_info);
            }
        }
        // TODO: handle token metadata
    }

    let genesis_hash = get_genesis_hash(pset);

    let params = SignLiquidTxParams {
        genesis_hash: genesis_hash.map(|h| h.as_byte_array().to_vec()),
        network,
        txn,
        num_inputs: tx.input.len() as u32,
        // LWK always uses Jade's anti-exfil signing flow. Jade defaults this field to false;
        // anti-exfil has been supported since firmware 0.1.24.
        use_ae_signatures: true,
        change: changes,
        asset_info: assets_info,
        trusted_commitments,
        additional_info: None,
    };
    Ok(params)
}

// Get a script from witness script pubkey hash
fn script_code_wpkh(script: &Script) -> Script {
    assert!(script.is_v0_p2wpkh());
    // ugly segwit stuff
    let mut script_code = vec![0x76u8, 0xa9, 0x14];
    script_code.extend(&script.as_bytes()[2..]);
    script_code.push(0x88);
    script_code.push(0xac);
    Script::from(script_code)
}

// taken and adapted from:
// https://github.com/rust-bitcoin/rust-bitcoin/blob/37daf4620c71dc9332c3e08885cf9de696204bca/bitcoin/src/blockdata/script/borrowed.rs#L266
// TODO remove once it's released
fn is_multisig(script: &Script) -> bool {
    fn decode_pushnum(op: All) -> Option<u8> {
        let start: u8 = OP_PUSHNUM_1.into_u8();
        let end: u8 = OP_PUSHNUM_16.into_u8();
        if start < op.into_u8() && end >= op.into_u8() {
            Some(op.into_u8() - start + 1)
        } else {
            None
        }
    }

    let required_sigs;

    let mut instructions = script.instructions();
    if let Some(Ok(Instruction::Op(op))) = instructions.next() {
        if let Some(pushnum) = decode_pushnum(op) {
            required_sigs = pushnum;
        } else {
            return false;
        }
    } else {
        return false;
    }

    let mut num_pubkeys: u8 = 0;
    while let Some(Ok(instruction)) = instructions.next() {
        match instruction {
            Instruction::PushBytes(_) => {
                num_pubkeys += 1;
            }
            Instruction::Op(op) => {
                if let Some(pushnum) = decode_pushnum(op) {
                    if pushnum != num_pubkeys {
                        return false;
                    }
                }
                break;
            }
        }
    }

    if required_sigs > num_pubkeys {
        return false;
    }

    if let Some(Ok(Instruction::Op(op))) = instructions.next() {
        if op != OP_CHECKMULTISIG {
            return false;
        }
    } else {
        return false;
    }

    instructions.next().is_none()
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use elements::{
        bitcoin::bip32::Fingerprint,
        confidential::Value,
        encode::serialize,
        pset::{Output, PartiallySignedTransaction},
        Script,
    };
    use lwk_common::Network;

    use crate::{create_jade_sign_req, is_multisig, json_to_cbor};

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

    #[test]
    fn test_is_multisig() {
        let multisig = Script::from_str("522102ebc62c20f1e09e169a88745f60f6dac878c92db5c7ed78c6703d2d0426a01f942102c2d59d677122bc292048833003fd5cb19d27d32896b1d0feec654c291f7ede9e52ae").unwrap();
        assert_eq!(multisig.asm(), "OP_PUSHNUM_2 OP_PUSHBYTES_33 02ebc62c20f1e09e169a88745f60f6dac878c92db5c7ed78c6703d2d0426a01f94 OP_PUSHBYTES_33 02c2d59d677122bc292048833003fd5cb19d27d32896b1d0feec654c291f7ede9e OP_PUSHNUM_2 OP_CHECKMULTISIG");
        assert!(is_multisig(&multisig));

        let not_multisig =
            Script::from_str("001414fe45f2c2a2b7c00d0940d694a3b6af6c9bf165").unwrap();
        assert_eq!(
            not_multisig.asm(),
            "OP_0 OP_PUSHBYTES_20 14fe45f2c2a2b7c00d0940d694a3b6af6c9bf165"
        );
        assert!(!is_multisig(&not_multisig));
    }

    #[test]
    fn create_jade_sign_req_skips_explicit_output_commitment() {
        let mut pset = PartiallySignedTransaction::new_v2();
        pset.add_output(Output::new_explicit(
            Script::from_str("001414fe45f2c2a2b7c00d0940d694a3b6af6c9bf165").unwrap(),
            1_000,
            *Network::default_regtest().policy_asset(),
            None,
        ));

        let params = create_jade_sign_req(
            &mut pset,
            Fingerprint::from([0u8; 4]),
            vec![],
            Network::default_regtest(),
        )
        .unwrap();

        assert_eq!(params.trusted_commitments.len(), 1);
        assert!(params.trusted_commitments[0].is_none());
    }

    #[test]
    fn explicit_input_value_serializes_to_jade_tx_input_format() {
        // Upstream Jade has a sign_liquid_tx fixture that passes an explicit
        // serialized input value as `value_commitment`:
        // https://github.com/Blockstream/Jade/blob/3edd8f4b03ae65d6ee38fb8620b46aad88ab341e/test_data/liquid_txn_nonconfidential_input.json#L54
        let explicit_value = serialize(&Value::Explicit(6_800_000));
        assert_eq!(explicit_value.len(), 9);
        assert_eq!(explicit_value[0], 1);
        assert_eq!(explicit_value, vec![1, 0, 0, 0, 0, 0, 103, 194, 128]);
    }

    fn response(id: &str, result: &str) -> crate::protocol::Response<String> {
        crate::protocol::Response {
            id: id.to_string(),
            result: Some(result.to_string()),
            error: None,
            seqnum: None,
            seqlen: None,
        }
    }

    #[test]
    fn try_parse_response_with_two_messages_in_the_buffer() {
        let mut buf = serde_cbor::to_vec(&response("1", "first")).unwrap();
        buf.extend_from_slice(&serde_cbor::to_vec(&response("2", "second")).unwrap());

        match crate::try_parse_response::<String>(&buf, "1") {
            crate::ParseStep::Mine(parsed) => {
                let parsed = parsed.unwrap();
                assert_eq!(parsed.id, "1");
                assert_eq!(parsed.result.as_deref(), Some("first"));
            }
            _ => panic!("two concatenated messages must not look like an incomplete one"),
        }
    }

    #[test]
    fn try_parse_response_skips_a_message_without_id() {
        let log = serde_cbor::to_vec(&serde_json::json!({"log": "boot"})).unwrap();

        let crate::ParseStep::Skip { consumed } = crate::try_parse_response::<String>(&log, "1")
        else {
            panic!("a message without an id must be skipped")
        };
        assert_eq!(consumed, log.len());
    }

    fn reject(id: &str) -> Vec<u8> {
        serde_cbor::to_vec(&serde_json::json!({
            "id": id,
            "error": {"code": -32600, "message": "Invalid RPC Request message", "data": "4096"}
        }))
        .unwrap()
    }

    #[test]
    fn try_parse_response_returns_the_unmatched_request_error() {
        let crate::ParseStep::Mine(parsed) =
            crate::try_parse_response::<String>(&reject(crate::UNMATCHED_REQUEST_ID), "1")
        else {
            panic!("an error about the request we just sent must not be skipped")
        };
        assert_eq!(
            parsed.unwrap_err().to_string(),
            "Jade Error: Error code: -32600 - message: Invalid RPC Request message"
        );
    }

    #[test]
    fn try_parse_response_skips_an_error_for_another_request() {
        let other = reject("2");

        let crate::ParseStep::Skip { consumed } = crate::try_parse_response::<String>(&other, "1")
        else {
            panic!("an error carrying another id must be skipped")
        };
        assert_eq!(consumed, other.len());
    }

    #[test]
    fn try_parse_response_skips_an_unmatched_message_without_an_error() {
        let odd =
            serde_cbor::to_vec(&serde_json::json!({"id": "00", "result": "unexpected"})).unwrap();

        let crate::ParseStep::Skip { consumed } = crate::try_parse_response::<String>(&odd, "1")
        else {
            panic!("id 00 without an error is not an answer for us")
        };
        assert_eq!(consumed, odd.len());
    }
}
