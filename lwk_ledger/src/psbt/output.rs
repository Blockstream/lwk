use elements_miniscript::elements::{
    pset::{self, raw, Output},
    TxOut,
};

/// Type: Redeem Script PSET_OUT_REDEEM_SCRIPT = 0x00
const PSET_OUT_REDEEM_SCRIPT: u8 = 0x00;
/// Type: Witness Script PSET_OUT_WITNESS_SCRIPT = 0x01
const PSET_OUT_WITNESS_SCRIPT: u8 = 0x01;
/// Type: BIP 32 Derivation Path PSET_OUT_BIP32_DERIVATION = 0x02
const PSET_OUT_BIP32_DERIVATION: u8 = 0x02;
/// Type: Output Amount PSET_OUT_AMOUNT = 0x03
const PSET_OUT_AMOUNT: u8 = 0x03;
/// Type: Output Script PSET_OUT_SCRIPT = 0x04
const PSET_OUT_SCRIPT: u8 = 0x04;
/// Type: Taproot Internal Key PSBT_OUT_TAP_INTERNAL_KEY = 0x05
const PSBT_OUT_TAP_INTERNAL_KEY: u8 = 0x05;
/// Type: Taproot Tree PSBT_OUT_TAP_TREE = 0x06
const PSBT_OUT_TAP_TREE: u8 = 0x06;
/// Type: Taproot Key BIP 32 Derivation Path PSBT_OUT_TAP_BIP32_DERIVATION = 0x07
const PSBT_OUT_TAP_BIP32_DERIVATION: u8 = 0x07;
/// Type: Proprietary Use Type PSET_IN_PROPRIETARY = 0xFC
//const PSET_OUT_PROPRIETARY: u8 = 0xFC;

/// Elements
/// The 33 byte Value Commitment for this output. This is mutually
/// exclusive with PSBT_OUT_VALUE.
const PSBT_ELEMENTS_OUT_VALUE_COMMITMENT: u8 = 0x01;
/// The explicit 32 byte asset tag for this output. This is mutually
/// exclusive with PSBT_ELEMENTS_OUT_ASSET_COMMITMENT.
const PSBT_ELEMENTS_OUT_ASSET: u8 = 0x02;
/// The 33 byte Asset Commitment for this output. This is mutually
/// exclusive with PSBT_ELEMENTS_OUT_ASSET.
const PSBT_ELEMENTS_OUT_ASSET_COMMITMENT: u8 = 0x03;
/// The rangeproof for the value of this output.
const PSBT_ELEMENTS_OUT_VALUE_RANGEPROOF: u8 = 0x04;
/// The asset surjection proof for this output's asset.
const PSBT_ELEMENTS_OUT_ASSET_SURJECTION_PROOF: u8 = 0x05;
/// The 33 byte blinding pubkey to be used when blinding this output.
const PSBT_ELEMENTS_OUT_BLINDING_PUBKEY: u8 = 0x06;
/// The 33 byte ephemeral pubkey used for ECDH in the blinding of this output.
const PSBT_ELEMENTS_OUT_ECDH_PUBKEY: u8 = 0x07;
/// The unsigned 32-bit little endian integer index of the input
/// whose owner should blind this output.
const PSBT_ELEMENTS_OUT_BLINDER_INDEX: u8 = 0x08;
/// An explicit value rangeproof that proves that the value commitment in
/// PSBT_ELEMENTS_OUT_VALUE_COMMITMENT matches the explicit value in PSBT_OUT_VALUE.
/// If provided, PSBT_ELEMENTS_OUT_VALUE_COMMITMENT must be provided too.
const PSBT_ELEMENTS_OUT_BLIND_VALUE_PROOF: u8 = 0x09;
/// An asset surjection proof with this output's asset as the only asset in the
/// input set in order to prove that the asset commitment in
/// PSBT_ELEMENTS_OUT_ASSET_COMMITMENT matches the explicit asset in
/// PSBT_ELEMENTS_OUT_ASSET. If provided, PSBT_ELEMENTS_OUT_ASSET_COMMITMENT must
/// be provided too.
const PSBT_ELEMENTS_OUT_BLIND_ASSET_PROOF: u8 = 0x0a;
/// The 32 byte asset blinding factor for this output.
const PSBT_ELEMENTS_OUT_ASSET_BLINDING_FACTOR: u8 = 0x0b;

pub fn get_v2_output_pairs(output: &Output, _txout: &TxOut) -> Vec<raw::Pair> {
    let mut rv: Vec<raw::Pair> = Default::default();

    impl_pset_get_pair! {
        rv.push(output.redeem_script as <PSET_OUT_REDEEM_SCRIPT, _>)
    }

    impl_pset_get_pair! {
        rv.push(output.witness_script as <PSET_OUT_WITNESS_SCRIPT, _>)
    }

    impl_pset_get_pair! {
        rv.push(output.bip32_derivation as <PSET_OUT_BIP32_DERIVATION, PublicKey>)
    }

    impl_pset_get_pair! {
        rv.push(output.tap_internal_key as <PSBT_OUT_TAP_INTERNAL_KEY, _>)
    }

    impl_pset_get_pair! {
        rv.push(output.tap_tree as <PSBT_OUT_TAP_TREE, _>)
    }

    impl_pset_get_pair! {
        rv.push(output.tap_key_origins as <PSBT_OUT_TAP_BIP32_DERIVATION,
                schnorr::PublicKey>)
    }

    impl_pset_get_pair! {
        rv.push(output.amount as <PSET_OUT_AMOUNT, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.amount_comm as <PSBT_ELEMENTS_OUT_VALUE_COMMITMENT, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.asset as <PSBT_ELEMENTS_OUT_ASSET, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.asset_comm as <PSBT_ELEMENTS_OUT_ASSET_COMMITMENT, _>)
    }

    // Mandatory field: Script
    rv.push(raw::Pair {
        key: raw::Key {
            type_value: PSET_OUT_SCRIPT,
            key: vec![],
        },
        value: pset::serialize::Serialize::serialize(&output.script_pubkey),
    });

    // Prop Output fields
    impl_pset_get_pair! {
        rv.push_prop(output.value_rangeproof as <PSBT_ELEMENTS_OUT_VALUE_RANGEPROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.asset_surjection_proof as <PSBT_ELEMENTS_OUT_ASSET_SURJECTION_PROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.blinding_key as <PSBT_ELEMENTS_OUT_BLINDING_PUBKEY, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.ecdh_pubkey as <PSBT_ELEMENTS_OUT_ECDH_PUBKEY, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.blinder_index as <PSBT_ELEMENTS_OUT_BLINDER_INDEX, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.blind_value_proof as <PSBT_ELEMENTS_OUT_BLIND_VALUE_PROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.blind_asset_proof as <PSBT_ELEMENTS_OUT_BLIND_ASSET_PROOF, _>)
    }

    impl_pset_get_pair! {
        rv.push_prop(output.asset_blinding_factor as <PSBT_ELEMENTS_OUT_ASSET_BLINDING_FACTOR, _>)
    }

    for (key, value) in output.proprietary.iter() {
        rv.push(raw::Pair {
            key: key.to_key(),
            value: value.clone(),
        });
    }

    for (key, value) in output.unknown.iter() {
        rv.push(raw::Pair {
            key: key.clone(),
            value: value.clone(),
        });
    }

    rv
}
