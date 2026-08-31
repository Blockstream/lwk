//! Inspection of PSET inputs.

use elements::{
    pset::{Input, PartiallySignedTransaction},
    EcdsaSighashType, SchnorrSighashType, Script,
};

fn input_spent_script_pubkey(input: &Input) -> Option<&Script> {
    if let Some(prevout) = input.witness_utxo.as_ref() {
        Some(&prevout.script_pubkey)
    } else if let Some(tx) = input.non_witness_utxo.as_ref() {
        tx.output
            .get(input.previous_output_index as usize)
            .map(|o| &o.script_pubkey)
    } else {
        None
    }
}

/// Return the sighash declared by an input, the default implied by the spent output script, or `None` if the spent output is unknown.
pub fn input_sighash(input: &Input) -> Option<u32> {
    if let Some(sighash) = input.sighash_type {
        return Some(sighash.to_u32());
    }

    input_spent_script_pubkey(input).map(|s| if s.is_v1_p2tr() { 0 } else { 1 })
}

fn input_has_non_default_sighash(input: &Input) -> bool {
    let Some(sighash) = input.sighash_type else {
        return false;
    };
    let Some(spk) = input_spent_script_pubkey(input) else {
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
    use elements::pset::PartiallySignedTransaction;

    use super::{input_sighash, pset_has_non_default_sighash};

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
    fn test_pset_has_non_default_sighash() {
        let pset_str = include_str!("../test_data/pset_details/pset.base64");
        let pset: PartiallySignedTransaction = pset_str.parse().unwrap();

        // an input declaring nothing commits to everything, whatever the spent output is
        assert!(!pset_has_non_default_sighash(&pset));

        let declare = |sighash: u32| {
            let mut pset = pset.clone();
            pset.inputs_mut()[0].sighash_type =
                Some(elements::pset::PsbtSighashType::from_u32(sighash));
            pset
        };

        // p2wsh input: only `ALL` commits to everything
        assert!(!pset_has_non_default_sighash(&declare(1)));
        assert!(pset_has_non_default_sighash(&declare(131)));

        // non standard values cannot be interpreted, warn about them
        assert!(pset_has_non_default_sighash(&declare(153)));

        let mut p2tr = vec![0x51, 0x20];
        p2tr.extend([0u8; 32]);
        let p2tr = elements::Script::from(p2tr);
        assert!(p2tr.is_v1_p2tr());
        let declare_taproot = |sighash: u32| {
            let mut pset = declare(sighash);
            pset.inputs_mut()[0]
                .witness_utxo
                .as_mut()
                .unwrap()
                .script_pubkey = p2tr.clone();
            pset
        };

        // taproot input: `DEFAULT` and `ALL` commit to the same data
        assert!(!pset_has_non_default_sighash(&declare_taproot(0)));
        assert!(!pset_has_non_default_sighash(&declare_taproot(1)));
        assert!(pset_has_non_default_sighash(&declare_taproot(3)));

        // without the spent output the script type is unknown, don't warn
        let mut pset = declare(131);
        pset.inputs_mut()[0].witness_utxo = None;
        assert!(!pset_has_non_default_sighash(&pset));
    }
}
