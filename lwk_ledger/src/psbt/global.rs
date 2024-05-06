use super::macros::u32_to_array_le;
use elements_miniscript::elements::{
    pset, pset::raw, pset::GlobalTxData, pset::PartiallySignedTransaction as Psbt, VarInt,
};

// (Not used in pset) Type: Unsigned Transaction PSET_GLOBAL_UNSIGNED_TX = 0x00
//const PSET_GLOBAL_UNSIGNED_TX: u8 = 0x00;
//
/// Type: Extended Public Key PSET_GLOBAL_XPUB = 0x01
const PSET_GLOBAL_XPUB: u8 = 0x01;

/// Type: Tx Version PSET_GLOBAL_TX_VERSION = 0x02
const PSET_GLOBAL_TX_VERSION: u8 = 0x02;
/// Type: Fallback Locktime PSET_GLOBAL_FALLBACK_LOCKTIME = 0x03
const PSET_GLOBAL_FALLBACK_LOCKTIME: u8 = 0x03;
/// Type: Tx Input Count PSET_GLOBAL_INPUT_COUNT = 0x04
const PSET_GLOBAL_INPUT_COUNT: u8 = 0x04;
/// Type: Tx Output Count PSET_GLOBAL_OUTPUT_COUNT = 0x05
const PSET_GLOBAL_OUTPUT_COUNT: u8 = 0x05;
/// Type: Transaction Modifiable Flags PSET_GLOBAL_TX_MODIFIABLE = 0x06
const PSET_GLOBAL_TX_MODIFIABLE: u8 = 0x06;

/// Type: Version Number PSET_GLOBAL_VERSION = 0xFB
const PSET_GLOBAL_VERSION: u8 = 0xFB;
/// Type: Proprietary Use Type PSET_GLOBAL_PROPRIETARY = 0xFC
//const PSET_GLOBAL_PROPRIETARY: u8 = 0xFC;

/// Proprietary fields in elements
/// Type: Global Scalars used in range proofs = 0x00
const PSBT_ELEMENTS_GLOBAL_SCALAR: u8 = 0x00;
/// Type: Global Flag used in elements for Blinding signalling
const PSBT_ELEMENTS_GLOBAL_TX_MODIFIABLE: u8 = 0x01;

pub fn get_v2_global_pairs(psbt: &Psbt) -> Vec<raw::Pair> {
    let mut rv: Vec<raw::Pair> = Default::default();

    let GlobalTxData {
        version,
        fallback_locktime,
        tx_modifiable,
        ..
    } = psbt.global.tx_data;
    let input_count_vint = VarInt(psbt.n_inputs() as u64);
    let output_count_vint = VarInt(psbt.n_outputs() as u64);

    impl_pset_get_pair! {
        rv.push_mandatory(version as <PSET_GLOBAL_TX_VERSION, _>)
    }

    impl_pset_get_pair! {
        rv.push(fallback_locktime as <PSET_GLOBAL_FALLBACK_LOCKTIME, _>)
    }

    impl_pset_get_pair! {
        rv.push_mandatory(input_count_vint as <PSET_GLOBAL_INPUT_COUNT, _>)
    }

    impl_pset_get_pair! {
        rv.push_mandatory(output_count_vint as <PSET_GLOBAL_OUTPUT_COUNT, _>)
    }

    impl_pset_get_pair! {
        rv.push(tx_modifiable as <PSET_GLOBAL_TX_MODIFIABLE, _>)
    }

    for (xpub, (fingerprint, derivation)) in &psbt.global.xpub {
        rv.push(raw::Pair {
            key: raw::Key {
                type_value: PSET_GLOBAL_XPUB,
                key: xpub.encode().to_vec(),
            },
            value: {
                let mut ret = Vec::with_capacity(4 + derivation.len() * 4);
                ret.extend(fingerprint.as_bytes());
                derivation
                    .into_iter()
                    .for_each(|n| ret.extend(&u32_to_array_le((*n).into())));
                ret
            },
        });
    }

    let ver = psbt.global.version; //hack to use macro
    impl_pset_get_pair!(
        rv.push_mandatory(ver as <PSET_GLOBAL_VERSION, _>)
    );

    // Serialize scalars and elements tx modifiable
    for scalar in &psbt.global.scalars {
        let key = raw::ProprietaryKey::from_pset_pair(
            PSBT_ELEMENTS_GLOBAL_SCALAR,
            scalar.as_ref().to_vec(),
        );
        rv.push(raw::Pair {
            key: key.to_key(),
            value: vec![], // This is a bug in elements core c++, parses this value as vec![0]
        })
    }

    let global = &psbt.global; // hack to use macro
    impl_pset_get_pair! {
        rv.push_prop(global.elements_tx_modifiable_flag as <PSBT_ELEMENTS_GLOBAL_TX_MODIFIABLE, _>)
    }

    for (key, value) in psbt.global.proprietary.iter() {
        rv.push(raw::Pair {
            key: key.to_key(),
            value: value.clone(),
        });
    }

    for (key, value) in psbt.global.unknown.iter() {
        rv.push(raw::Pair {
            key: key.clone(),
            value: value.clone(),
        });
    }

    rv
}
