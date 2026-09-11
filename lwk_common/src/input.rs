//! Inspection of PSET inputs.

use elements::{
    pset::{Input, PartiallySignedTransaction},
    EcdsaSighashType, SchnorrSighashType, TxOut,
};

pub(crate) fn spent_txout(input: &Input) -> Option<&TxOut> {
    if let Some(witness_utxo) = input.witness_utxo.as_ref() {
        Some(witness_utxo)
    } else if let Some(tx) = input.non_witness_utxo.as_ref() {
        tx.output.get(input.previous_output_index as usize)
    } else {
        None
    }
}

/// Whether the input spends a v1 p2tr output.
pub fn is_taproot_input(input: &Input) -> bool {
    spent_txout(input)
        .map(|o| o.script_pubkey.is_v1_p2tr())
        .unwrap_or(false)
}

/// Return the sighash declared by an input, the default implied by the spent output script, or `None` if the spent output is unknown.
pub fn input_sighash(input: &Input) -> Option<u32> {
    if let Some(sighash) = input.sighash_type {
        return Some(sighash.to_u32());
    }

    spent_txout(input).map(|o| if o.script_pubkey.is_v1_p2tr() { 0 } else { 1 })
}

/// Whether the input declares a sighash type that is not the default one.
///
/// If the spent output is missing, returns `false`.
pub fn input_has_non_default_sighash(input: &Input) -> bool {
    let Some(sighash) = input.sighash_type else {
        return false;
    };
    let Some(spk) = spent_txout(input).map(|o| &o.script_pubkey) else {
        return false;
    };

    if spk.is_v1_p2tr() {
        // SIGHASH_DEFAULT and SIGHASH_ALL commit to the same data, but only the former can be
        // implied, so a taproot input declaring SIGHASH_ALL is still a default one.
        !matches!(
            sighash.schnorr_hash_ty(),
            Some(SchnorrSighashType::Default) | Some(SchnorrSighashType::All)
        )
    } else {
        !matches!(sighash.ecdsa_hash_ty(), Some(EcdsaSighashType::All))
    }
}

/// Whether any PSET input sighash is not the default one.
pub(crate) fn pset_has_non_default_sighash(pset: &PartiallySignedTransaction) -> bool {
    pset.inputs().iter().any(input_has_non_default_sighash)
}

#[cfg(test)]
mod test {
    use elements::pset::{Input, PartiallySignedTransaction, PsbtSighashType};
    use elements::{EcdsaSighashType, LockTime, SchnorrSighashType, Script, Transaction, TxOut};

    use super::{input_has_non_default_sighash, input_sighash, pset_has_non_default_sighash};

    #[test]
    fn test_input_sighash() {
        let pset_str = include_str!("../test_data/pset_details/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        let mut input = pset.inputs()[0].clone();

        // p2wsh input declaring nothing defaults to `ALL`
        assert!(input.sighash_type.is_none());
        assert_eq!(input_sighash(&input), Some(1));

        // the declared value is reported as is, even when non standard
        input.sighash_type = Some(elements::pset::PsbtSighashType::from_u32(131));
        assert_eq!(input_sighash(&input), Some(131));
        input.sighash_type = Some(elements::pset::PsbtSighashType::from_u32(153));
        assert_eq!(input_sighash(&input), Some(153));

        // a taproot input declaring nothing defaults to `DEFAULT`
        input.sighash_type = None;
        let mut p2tr = vec![0x51, 0x20];
        p2tr.extend([0u8; 32]);
        let p2tr = elements::Script::from(p2tr);
        assert!(p2tr.is_v1_p2tr());
        input.witness_utxo.as_mut().unwrap().script_pubkey = p2tr.clone();
        assert_eq!(input_sighash(&input), Some(0));

        // the spent output is taken from `non_witness_utxo` when there is no `witness_utxo`
        let mut tx = elements::Transaction {
            version: 2,
            lock_time: elements::LockTime::ZERO,
            input: vec![],
            output: vec![elements::TxOut::default(), elements::TxOut::default()],
        };
        tx.output[1].script_pubkey = p2tr;
        input.witness_utxo = None;
        input.non_witness_utxo = Some(tx);
        input.previous_output_index = 1;
        assert_eq!(input_sighash(&input), Some(0));

        // without any spent output the input cannot be signed, return None
        input.non_witness_utxo = None;
        assert_eq!(input_sighash(&input), None);
    }

    #[test]
    fn test_non_default_sighash() {
        let p2tr = Script::from([&[0x51, 0x20][..], &[0u8; 32]].concat());
        let txout = |script_pubkey: &Script| TxOut {
            script_pubkey: script_pubkey.clone(),
            ..Default::default()
        };

        let mut ecdsa = Input {
            witness_utxo: Some(txout(&Script::new())),
            ..Default::default()
        };
        let mut tr = Input {
            witness_utxo: Some(txout(&p2tr)),
            ..Default::default()
        };
        let mut tr_non_witness = Input {
            non_witness_utxo: Some(Transaction {
                version: 2,
                lock_time: LockTime::ZERO,
                input: vec![],
                output: vec![txout(&p2tr)],
            }),
            ..Default::default()
        };
        let mut unknown = Input::default();

        let ecdsa_def = [EcdsaSighashType::All];
        let ecdsa_non_def = [
            EcdsaSighashType::None,
            EcdsaSighashType::Single,
            EcdsaSighashType::AllPlusAnyoneCanPay,
            EcdsaSighashType::NonePlusAnyoneCanPay,
            EcdsaSighashType::SinglePlusAnyoneCanPay,
        ];
        let tr_def = [SchnorrSighashType::Default, SchnorrSighashType::All];
        let tr_non_def = [
            SchnorrSighashType::None,
            SchnorrSighashType::Single,
            SchnorrSighashType::AllPlusAnyoneCanPay,
            SchnorrSighashType::NonePlusAnyoneCanPay,
            SchnorrSighashType::SinglePlusAnyoneCanPay,
        ];

        // nothing declared
        for input in [&ecdsa, &tr, &tr_non_witness, &unknown] {
            assert!(!input_has_non_default_sighash(input));
        }

        for sighash in ecdsa_def {
            ecdsa.sighash_type = Some(sighash.into());
            assert!(!input_has_non_default_sighash(&ecdsa), "{sighash}");
        }
        for sighash in ecdsa_non_def {
            ecdsa.sighash_type = Some(sighash.into());
            assert!(input_has_non_default_sighash(&ecdsa), "{sighash}");
        }
        for input in [&mut tr, &mut tr_non_witness] {
            for sighash in tr_def {
                input.sighash_type = Some(sighash.into());
                assert!(!input_has_non_default_sighash(input), "{sighash}");
            }
            for sighash in tr_non_def {
                input.sighash_type = Some(sighash.into());
                assert!(input_has_non_default_sighash(input), "{sighash}");
            }
        }

        // `DEFAULT` is taproot only
        ecdsa.sighash_type = Some(SchnorrSighashType::Default.into());
        assert!(input_has_non_default_sighash(&ecdsa));

        // non standard values cannot be interpreted
        for input in [&mut ecdsa, &mut tr] {
            input.sighash_type = Some(PsbtSighashType::from_u32(153));
            assert!(input_has_non_default_sighash(input));
        }

        // without the spent output value is not interpreted
        unknown.sighash_type = Some(SchnorrSighashType::Default.into());
        assert!(!input_has_non_default_sighash(&unknown));
        for sighash in ecdsa_def.into_iter().chain(ecdsa_non_def) {
            unknown.sighash_type = Some(sighash.into());
            assert!(!input_has_non_default_sighash(&unknown), "{sighash}");
        }

        // a pset is non default as soon as one of its inputs is
        let pset_str = include_str!("../test_data/pset_details/pset.base64");
        let mut pset: PartiallySignedTransaction = pset_str.parse().unwrap();
        assert!(!pset_has_non_default_sighash(&pset));
        pset.inputs_mut()[0].sighash_type = Some(EcdsaSighashType::SinglePlusAnyoneCanPay.into());
        assert!(pset_has_non_default_sighash(&pset));
    }
}
