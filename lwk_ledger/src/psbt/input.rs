use elements_miniscript::elements::{
    pset::{self, raw, Input},
    TxIn,
};

/// Type: Non-Witness UTXO PSET_IN_NON_WITNESS_UTXO = 0x00
const PSET_IN_NON_WITNESS_UTXO: u8 = 0x00;
/// Type: Witness UTXO PSET_IN_WITNESS_UTXO = 0x01
const PSET_IN_WITNESS_UTXO: u8 = 0x01;
/// Type: Partial Signature PSET_IN_PARTIAL_SIG = 0x02
const PSET_IN_PARTIAL_SIG: u8 = 0x02;
/// Type: Sighash Type PSET_IN_SIGHASH_TYPE = 0x03
const PSET_IN_SIGHASH_TYPE: u8 = 0x03;
/// Type: Redeem Script PSET_IN_REDEEM_SCRIPT = 0x04
const PSET_IN_REDEEM_SCRIPT: u8 = 0x04;
/// Type: Witness Script PSET_IN_WITNESS_SCRIPT = 0x05
const PSET_IN_WITNESS_SCRIPT: u8 = 0x05;
/// Type: BIP 32 Derivation Path PSET_IN_BIP32_DERIVATION = 0x06
const PSET_IN_BIP32_DERIVATION: u8 = 0x06;
/// Type: Finalized scriptSig PSET_IN_FINAL_SCRIPTSIG = 0x07
const PSET_IN_FINAL_SCRIPTSIG: u8 = 0x07;
/// Type: Finalized scriptWitness PSET_IN_FINAL_SCRIPTWITNESS = 0x08
const PSET_IN_FINAL_SCRIPTWITNESS: u8 = 0x08;
/// Type: RIPEMD160 preimage PSET_IN_RIPEMD160 = 0x0a
const PSET_IN_RIPEMD160: u8 = 0x0a;
/// Type: SHA256 preimage PSET_IN_SHA256 = 0x0b
const PSET_IN_SHA256: u8 = 0x0b;
/// Type: HASH160 preimage PSET_IN_HASH160 = 0x0c
const PSET_IN_HASH160: u8 = 0x0c;
/// Type: HASH256 preimage PSET_IN_HASH256 = 0x0d
const PSET_IN_HASH256: u8 = 0x0d;
/// Type: (Mandatory) Previous TXID PSET_IN_PREVIOUS_TXID = 0x0e
const PSET_IN_PREVIOUS_TXID: u8 = 0x0e;
/// Type: (Mandatory) Spent Output Index PSET_IN_OUTPUT_INDEX = 0x0f
const PSET_IN_OUTPUT_INDEX: u8 = 0x0f;
/// Type: Sequence Number PSET_IN_SEQUENCE = 0x10
const PSET_IN_SEQUENCE: u8 = 0x10;
/// Type: Required Time-based Locktime PSET_IN_REQUIRED_TIME_LOCKTIME = 0x11
const PSET_IN_REQUIRED_TIME_LOCKTIME: u8 = 0x11;
/// Type: Required Height-based Locktime PSET_IN_REQUIRED_HEIGHT_LOCKTIME = 0x12
const PSET_IN_REQUIRED_HEIGHT_LOCKTIME: u8 = 0x12;
/// Type: Schnorr Signature in Key Spend PSBT_IN_TAP_KEY_SIG = 0x13
const PSBT_IN_TAP_KEY_SIG: u8 = 0x13;
/// Type: Schnorr Signature in Script Spend PSBT_IN_TAP_SCRIPT_SIG = 0x14
const PSBT_IN_TAP_SCRIPT_SIG: u8 = 0x14;
/// Type: Taproot Leaf Script PSBT_IN_TAP_LEAF_SCRIPT = 0x14
const PSBT_IN_TAP_LEAF_SCRIPT: u8 = 0x15;
/// Type: Taproot Key BIP 32 Derivation Path PSBT_IN_TAP_BIP32_DERIVATION = 0x16
const PSBT_IN_TAP_BIP32_DERIVATION: u8 = 0x16;
/// Type: Taproot Internal Key PSBT_IN_TAP_INTERNAL_KEY = 0x17
const PSBT_IN_TAP_INTERNAL_KEY: u8 = 0x17;
/// Type: Taproot Merkle Root PSBT_IN_TAP_MERKLE_ROOT = 0x18
const PSBT_IN_TAP_MERKLE_ROOT: u8 = 0x18;
/// Type: Proprietary Use Type PSET_IN_PROPRIETARY = 0xFC
//const PSET_IN_PROPRIETARY: u8 = 0xFC;

// Elements Proprietary types:
/// Issuance Value: The explicit little endian 64-bit integer
/// for the value of this issuance. This is mutually exclusive with
/// PSBT_ELEMENTS_IN_ISSUANCE_VALUE_COMMITMENT
const PSBT_ELEMENTS_IN_ISSUANCE_VALUE: u8 = 0x00;
/// Issuance Value Commitment: The 33 byte Value Commitment.
/// This is mutually exclusive with PSBT_IN_ISSUANCE_VALUE.
const PSBT_ELEMENTS_IN_ISSUANCE_VALUE_COMMITMENT: u8 = 0x01;
/// Issuance Value Rangeproof: The rangeproof
const PSBT_ELEMENTS_IN_ISSUANCE_VALUE_RANGEPROOF: u8 = 0x02;
/// Issuance Inflation Keys Rangeproof: The rangeproof
const PSBT_ELEMENTS_IN_ISSUANCE_KEYS_RANGEPROOF: u8 = 0x03;
/// Peg-in Transaction: The Peg-in Transaction serialized without witnesses.
const PSBT_ELEMENTS_IN_PEG_IN_TX: u8 = 0x04;
/// Peg-in Transaction Output Proof: The transaction output proof for the
/// Peg-in Transaction.
const PSBT_ELEMENTS_IN_PEG_IN_TXOUT_PROOF: u8 = 0x05;
/// Peg-in Genesis Hash: The 32 byte genesis hash for the Peg-in Transaction.
const PSBT_ELEMENTS_IN_PEG_IN_GENESIS: u8 = 0x06;
/// Peg-in Claim Script: The claim script for the Peg-in Transaction.
const PSBT_ELEMENTS_IN_PEG_IN_CLAIM_SCRIPT: u8 = 0x07;
/// Peg-in Value: The little endian 64-bit value of the peg-in for
/// the Peg-in Transaction.
const PSBT_ELEMENTS_IN_PEG_IN_VALUE: u8 = 0x08;
/// Peg-in Witness: The Peg-in witness for the Peg-in Transaction.
const PSBT_ELEMENTS_IN_PEG_IN_WITNESS: u8 = 0x09;
/// Issuance Inflation Keys Amount: The value for the inflation keys output to
/// set in this issuance. This is mutually exclusive with
/// PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS_COMMITMENT.
const PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS: u8 = 0x0a;
/// Issuance Inflation Keys Amount Commitment: The 33 byte commitment to the
/// inflation keys output value in this issuance. This is mutually exclusive
/// with PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS
const PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS_COMMITMENT: u8 = 0x0b;
/// Issuance Blinding Nonce: The 32 byte asset blinding nonce. For new assets,
/// must be 0. For reissuances, this is a revelation of the blinding factor for
/// the input.
const PSBT_ELEMENTS_IN_ISSUANCE_BLINDING_NONCE: u8 = 0x0c;
/// Issuance Asset Entropy: The 32 byte asset entropy. For new issuances, an
/// arbitrary and optional 32 bytes of no consensus meaning combined used as
/// additional entropy in the asset tag calculation. For reissuances, the
/// original, final entropy used for the asset tag calculation.
const PSBT_ELEMENTS_IN_ISSUANCE_ASSET_ENTROPY: u8 = 0x0d;
/// The rangeproof for the UTXO for this input. This rangeproof is found in
/// the output witness data for the transaction and thus is not included as part
/// of either of the UTXOs (as witness data is not included in either case).
/// However the rangeproof is needed in order for stateless blinders to learn
/// the blinding factors for the UTXOs that they are involved in.
const PSBT_ELEMENTS_IN_UTXO_RANGEPROOF: u8 = 0x0e;
/// An explicit value rangeproof that proves that the value commitment in
/// PSBT_ELEMENTS_IN_ISSUANCE_VALUE_COMMITMENT matches the explicit value in
/// PSBT_ELEMENTS_IN_ISSUANCE_VALUE. If provided, PSBT_ELEMENTS_IN_ISSUANCE_VALUE_COMMITMENT
/// must be provided too.
const PSBT_ELEMENTS_IN_ISSUANCE_BLIND_VALUE_PROOF: u8 = 0x0f;
/// An explicit value rangeproof that proves that the value commitment in
/// PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS_COMMITMENT matches the explicit value
/// in PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS. If provided,
/// PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS_COMMITMENT must be provided too.
const PSBT_ELEMENTS_IN_ISSUANCE_BLIND_INFLATION_KEYS_PROOF: u8 = 0x10;
/// The explicit value for the input being spent. If provided,
/// PSBT_ELEMENTS_IN_VALUE_PROOF must be provided too.
const PSBT_ELEMENTS_IN_EXPLICIT_VALUE: u8 = 0x11;
/// An explicit value rangeproof that proves that the value commitment in this
/// input's UTXO matches the explicit value in PSBT_ELEMENTS_IN_EXPLICIT_VALUE.
/// If provided, PSBT_ELEMENTS_IN_EXPLICIT_VALUE must be provided too.
const PSBT_ELEMENTS_IN_VALUE_PROOF: u8 = 0x12;
/// The explicit asset for the input being spent. If provided,
/// PSBT_ELEMENTS_IN_ASSET_PROOF must be provided too.
const PSBT_ELEMENTS_IN_EXPLICIT_ASSET: u8 = 0x13;
/// An asset surjection proof with this input's asset as the only asset in the
/// input set in order to prove that the asset commitment in the UTXO matches
/// the explicit asset in PSBT_ELEMENTS_IN_EXPLICIT_ASSET. If provided,
/// PSBT_ELEMENTS_IN_EXPLICIT_ASSET must be provided too.
const PSBT_ELEMENTS_IN_ASSET_PROOF: u8 = 0x14;
/// A boolean flag. 0x00 indicates the issuance should not be blinded,
/// 0x01 indicates it should be. If not specified, assumed to be 0x01.
/// Note that this does not indicate actual blinding status,
/// but rather the expected blinding status prior to signing.
const PSBT_ELEMENTS_IN_BLINDED_ISSUANCE: u8 = 0x15;
/// The 32 byte asset blinding factor for the input being spent.
const PSBT_ELEMENTS_IN_ASSET_BLINDING_FACTOR: u8 = 0x16;

pub fn get_v2_input_pairs(input: &Input, _txin: &TxIn) -> Vec<raw::Pair> {
    let mut rv: Vec<raw::Pair> = Default::default();

    impl_pset_get_pair! {
        rv.push(input.non_witness_utxo as <PSET_IN_NON_WITNESS_UTXO, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.witness_utxo as <PSET_IN_WITNESS_UTXO, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.partial_sigs as <PSET_IN_PARTIAL_SIG, PublicKey>)
    }

    impl_pset_get_pair! {
        rv.push(input.sighash_type as <PSET_IN_SIGHASH_TYPE, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.redeem_script as <PSET_IN_REDEEM_SCRIPT, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.witness_script as <PSET_IN_WITNESS_SCRIPT, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.bip32_derivation as <PSET_IN_BIP32_DERIVATION, PublicKey>)
    }

    impl_pset_get_pair! {
        rv.push(input.final_script_sig as <PSET_IN_FINAL_SCRIPTSIG, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.final_script_witness as <PSET_IN_FINAL_SCRIPTWITNESS, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.ripemd160_preimages as <PSET_IN_RIPEMD160, ripemd160::Hash>)
    }

    impl_pset_get_pair! {
        rv.push(input.sha256_preimages as <PSET_IN_SHA256, sha256::Hash>)
    }

    impl_pset_get_pair! {
        rv.push(input.hash160_preimages as <PSET_IN_HASH160, hash160::Hash>)
    }

    impl_pset_get_pair! {
        rv.push(input.hash256_preimages as <PSET_IN_HASH256, sha256d::Hash>)
    }

    // Mandatory field: Prev Txid
    rv.push(raw::Pair {
        key: raw::Key {
            type_value: PSET_IN_PREVIOUS_TXID,
            key: vec![],
        },
        value: pset::serialize::Serialize::serialize(&input.previous_txid),
    });

    // Mandatory field: prev out index
    rv.push(raw::Pair {
        key: raw::Key {
            type_value: PSET_IN_OUTPUT_INDEX,
            key: vec![],
        },
        value: pset::serialize::Serialize::serialize(&input.previous_output_index),
    });

    impl_pset_get_pair! {
        rv.push(input.sequence as <PSET_IN_SEQUENCE, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.required_time_locktime as <PSET_IN_REQUIRED_TIME_LOCKTIME, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.required_height_locktime as <PSET_IN_REQUIRED_HEIGHT_LOCKTIME, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.tap_key_sig as <PSBT_IN_TAP_KEY_SIG, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.tap_script_sigs as <PSBT_IN_TAP_SCRIPT_SIG, (schnorr::PublicKey, TapLeafHash)>)
    }

    impl_pset_get_pair! {
        rv.push(input.tap_scripts as <PSBT_IN_TAP_LEAF_SCRIPT, ControlBlock>)
    }

    impl_pset_get_pair! {
        rv.push(input.tap_key_origins as <PSBT_IN_TAP_BIP32_DERIVATION,
            schnorr::PublicKey>)
    }

    impl_pset_get_pair! {
        rv.push(input.tap_internal_key as <PSBT_IN_TAP_INTERNAL_KEY, _>)
    }

    impl_pset_get_pair! {
        rv.push(input.tap_merkle_root as <PSBT_IN_TAP_MERKLE_ROOT, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.issuance_value_amount as <PSBT_ELEMENTS_IN_ISSUANCE_VALUE, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.issuance_value_comm as <PSBT_ELEMENTS_IN_ISSUANCE_VALUE_COMMITMENT, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.issuance_value_rangeproof as <PSBT_ELEMENTS_IN_ISSUANCE_VALUE_RANGEPROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.issuance_keys_rangeproof as <PSBT_ELEMENTS_IN_ISSUANCE_KEYS_RANGEPROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.pegin_tx as <PSBT_ELEMENTS_IN_PEG_IN_TX, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.pegin_txout_proof as <PSBT_ELEMENTS_IN_PEG_IN_TXOUT_PROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.pegin_genesis_hash as <PSBT_ELEMENTS_IN_PEG_IN_GENESIS, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.pegin_claim_script as <PSBT_ELEMENTS_IN_PEG_IN_CLAIM_SCRIPT, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.pegin_value as <PSBT_ELEMENTS_IN_PEG_IN_VALUE, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.pegin_witness as <PSBT_ELEMENTS_IN_PEG_IN_WITNESS, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.issuance_inflation_keys as <PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.issuance_inflation_keys_comm as <PSBT_ELEMENTS_IN_ISSUANCE_INFLATION_KEYS_COMMITMENT, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.issuance_blinding_nonce as <PSBT_ELEMENTS_IN_ISSUANCE_BLINDING_NONCE, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.issuance_asset_entropy as <PSBT_ELEMENTS_IN_ISSUANCE_ASSET_ENTROPY, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.in_utxo_rangeproof as <PSBT_ELEMENTS_IN_UTXO_RANGEPROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.in_issuance_blind_value_proof as <PSBT_ELEMENTS_IN_ISSUANCE_BLIND_VALUE_PROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.in_issuance_blind_inflation_keys_proof as <PSBT_ELEMENTS_IN_ISSUANCE_BLIND_INFLATION_KEYS_PROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.amount as <PSBT_ELEMENTS_IN_EXPLICIT_VALUE, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.blind_value_proof as <PSBT_ELEMENTS_IN_VALUE_PROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.asset as <PSBT_ELEMENTS_IN_EXPLICIT_ASSET, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.blind_asset_proof as <PSBT_ELEMENTS_IN_ASSET_PROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.blinded_issuance as <PSBT_ELEMENTS_IN_BLINDED_ISSUANCE, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(input.asset_blinding_factor as <PSBT_ELEMENTS_IN_ASSET_BLINDING_FACTOR, _>)
    }

    for (key, value) in input.proprietary.iter() {
        rv.push(raw::Pair {
            key: key.to_key(),
            value: value.clone(),
        });
    }

    for (key, value) in input.unknown.iter() {
        rv.push(raw::Pair {
            key: key.clone(),
            value: value.clone(),
        });
    }

    rv
}
